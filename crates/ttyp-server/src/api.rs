//! HTTP API (see docs/online-handoff.md §5). Handlers are thin: they
//! authenticate, call into `daily`/`leaderboard`/`auth`, and shape JSON.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use sqlx::Row;
use sqlx::sqlite::SqlitePool;
use ttyp_core::api::{
    Account, AccountUpdate, AuthRequest, AuthResponse, Board, Daily, DailySummary, ErrorBody,
    FollowUpdate, Leaderboard, PlayerList, PlayerSummary, Profile, ResultDetail, StartResponse,
    SubmitRequest, SubmitResponse,
};
use ttyp_core::boards::Period;
use ttyp_core::language::LanguageRegistry;
use ttyp_core::test::{Rejected, replay};

use crate::auth::{self, User};
use crate::leaderboard::Scope;
use crate::players::FollowError;
use crate::{daily, leaderboard, players, profile};

/// Attempts (starts and submissions) per user per daily.
const MAX_ATTEMPTS: i64 = 30;
const MAX_PAGE: u32 = 200;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub languages: Arc<LanguageRegistry>,
    /// Base URL of the GitHub REST API (`TTYP_GITHUB_API`; tests mock it).
    pub github_api: String,
}

impl AppState {
    pub fn new(pool: SqlitePool, languages: Arc<LanguageRegistry>) -> Self {
        let github_api =
            std::env::var("TTYP_GITHUB_API").unwrap_or_else(|_| "https://api.github.com".into());
        Self {
            pool,
            languages,
            github_api,
        }
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/dailies", get(dailies_on))
        .route("/dailies/today", get(dailies_today))
        .route("/dailies/{id}", get(daily_by_id))
        .route("/dailies/{id}/start", post(start))
        .route("/results", post(submit))
        .route("/results/{id}", get(result_by_id))
        .route("/leaderboard/{daily_id}", get(leaderboard))
        .route("/boards/{id}", get(board))
        .route("/account", get(account).post(update_account))
        .route("/users", get(search_players))
        .route("/users/{login}", get(user_profile))
        .route("/follows", get(follow_list).post(update_follow))
        .route("/auth/github", post(auth_github))
        .route("/auth/logout", post(auth_logout))
        .with_state(state)
}

/// Every failure becomes `{"error": …}` with a fitting status.
#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    Unauthorized,
    NotFound,
    TooMany,
    Internal(anyhow::Error),
}

impl<E: Into<anyhow::Error>> From<E> for ApiError {
    fn from(e: E) -> Self {
        ApiError::Internal(e.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            ApiError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, "login required".into()),
            ApiError::NotFound => (StatusCode::NOT_FOUND, "not found".into()),
            ApiError::TooMany => (StatusCode::TOO_MANY_REQUESTS, "too many attempts".into()),
            ApiError::Internal(e) => {
                tracing::error!("{e:#}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal error".into())
            }
        };
        (status, Json(ErrorBody { error: msg })).into_response()
    }
}

type ApiResult<T> = Result<Json<T>, ApiError>;

async fn require_user(state: &AppState, headers: &HeaderMap) -> Result<User, ApiError> {
    auth::user_from_headers(&state.pool, headers)
        .await?
        .ok_or(ApiError::Unauthorized)
}

async fn health(State(state): State<AppState>) -> ApiResult<serde_json::Value> {
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM daily_tests WHERE date = ?1")
        .bind(daily::today().to_string())
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(
        serde_json::json!({ "status": "ok", "dailies_today": n }),
    ))
}

async fn dailies_today(State(state): State<AppState>) -> ApiResult<Vec<DailySummary>> {
    let today = daily::today();
    daily::ensure(&state.pool, &state.languages, today).await?;
    Ok(Json(daily::list(&state.pool, today).await?))
}

#[derive(Debug, Deserialize)]
struct DailiesQuery {
    /// UTC date `YYYY-MM-DD`; defaults to today.
    date: Option<String>,
}

/// The dailies of one day. Only today is backfilled; other days are
/// whatever was generated then.
async fn dailies_on(
    State(state): State<AppState>,
    Query(q): Query<DailiesQuery>,
) -> ApiResult<Vec<DailySummary>> {
    let today = daily::today();
    let date = match q.date {
        None => today,
        Some(d) => chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d")
            .map_err(|_| ApiError::BadRequest("date must be YYYY-MM-DD".into()))?,
    };
    if date == today {
        daily::ensure(&state.pool, &state.languages, today).await?;
    }
    Ok(Json(daily::list(&state.pool, date).await?))
}

async fn daily_by_id(State(state): State<AppState>, Path(id): Path<i64>) -> ApiResult<Daily> {
    daily::get(&state.pool, id)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound)
}

/// The attempt number the user's next start or submission on `daily_id`
/// gets: one past every attempt so far, started or submitted. Call inside
/// the write transaction.
async fn next_attempt(
    tx: &mut sqlx::SqliteConnection,
    user_id: i64,
    daily_id: i64,
) -> Result<i64, ApiError> {
    let last: i64 = sqlx::query_scalar(
        "SELECT max( \
           (SELECT coalesce(max(attempt), 0) FROM starts WHERE user_id = ?1 AND daily_id = ?2), \
           (SELECT coalesce(max(attempt), 0) FROM results WHERE user_id = ?1 AND daily_id = ?2))",
    )
    .bind(user_id)
    .bind(daily_id)
    .fetch_one(&mut *tx)
    .await?;
    if last >= MAX_ATTEMPTS {
        return Err(ApiError::TooMany);
    }
    Ok(last + 1)
}

/// The first key of a daily was typed: that's an attempt, whether or not
/// a result ever arrives. Restarting can't hide a bad first try.
async fn start(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> ApiResult<StartResponse> {
    let user = require_user(&state, &headers).await?;
    daily::get(&state.pool, id)
        .await?
        .ok_or(ApiError::NotFound)?;
    let mut tx = state.pool.begin_with("BEGIN IMMEDIATE").await?;
    let attempt = next_attempt(&mut tx, user.id, id).await?;
    let done = sqlx::query(
        "INSERT INTO starts (user_id, daily_id, attempt, created_at) VALUES (?1, ?2, ?3, ?4)",
    )
    .bind(user.id)
    .bind(id)
    .bind(attempt)
    .bind(crate::db::now())
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(StartResponse {
        start_id: done.last_insert_rowid(),
        attempt: attempt as u32,
    }))
}

async fn submit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<SubmitRequest>,
) -> ApiResult<SubmitResponse> {
    let user = require_user(&state, &headers).await?;
    let daily = daily::get(&state.pool, req.daily_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("unknown daily".into()))?;
    let keylog = serde_json::to_vec(&req.keylog)?;

    // Only the server's replay counts; the log is kept for re-scoring.
    let (metrics, rejected) = match replay(&daily.words, daily.mode, &req.keylog) {
        Ok(m) => (Some(m), None),
        Err(e) => (None, Some(e)),
    };
    let (wpm, raw, acc, consistency) = metrics.as_ref().map_or((0.0, 0.0, 0.0, 0.0), |m| {
        (m.wpm, m.raw, m.accuracy, m.consistency)
    });
    let series = |m: Option<&ttyp_core::test::Metrics>,
                  f: fn(&ttyp_core::test::Metrics) -> String| {
        m.map_or_else(|| "[]".to_string(), f)
    };
    let chars = metrics.as_ref().map_or_else(
        || "{}".to_string(),
        |m| serde_json::to_string(&m.chars).unwrap_or_default(),
    );

    // Attempt numbering and the insert happen under one write lock. A run
    // with a start takes that start's number; one without (logged out while
    // typing, or an old client) gets the next number and can't be a first
    // try, since nothing proves earlier tries weren't thrown away.
    let mut tx = state.pool.begin_with("BEGIN IMMEDIATE").await?;
    let (attempt, first_eligible) = match req.start_id {
        Some(start_id) => {
            let row: Option<(i64, i64)> = sqlx::query_as(
                "SELECT s.attempt, (SELECT count(*) FROM results r WHERE r.start_id = s.id) \
                 FROM starts s WHERE s.id = ?1 AND s.user_id = ?2 AND s.daily_id = ?3",
            )
            .bind(start_id)
            .bind(user.id)
            .bind(daily.id)
            .fetch_optional(&mut *tx)
            .await?;
            match row {
                None => return Err(ApiError::BadRequest("unknown start".into())),
                Some((_, used)) if used > 0 => {
                    return Err(ApiError::BadRequest("run already submitted".into()));
                }
                Some((attempt, _)) => (attempt, true),
            }
        }
        None => (next_attempt(&mut tx, user.id, daily.id).await?, false),
    };
    let done = sqlx::query(
        "INSERT INTO results (user_id, daily_id, attempt, wpm, raw, acc, consistency, chars, \
         wpm_per_second, raw_per_second, errors_per_second, keylog, valid, created_at, \
         start_id, first_eligible) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
    )
    .bind(user.id)
    .bind(daily.id)
    .bind(attempt)
    .bind(wpm)
    .bind(raw)
    .bind(acc)
    .bind(consistency)
    .bind(chars)
    .bind(series(metrics.as_ref(), |m| json(&m.wpm_per_second)))
    .bind(series(metrics.as_ref(), |m| json(&m.raw_per_second)))
    .bind(series(metrics.as_ref(), |m| json(&m.errors_per_second)))
    .bind(keylog)
    .bind(i64::from(metrics.is_some()))
    .bind(crate::db::now())
    .bind(req.start_id)
    .bind(i64::from(first_eligible))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    let result_id = done.last_insert_rowid();

    let scope = Scope::Daily(daily.id);
    let follows_anyone: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM follows WHERE follower_id = ?1)")
            .bind(user.id)
            .fetch_one(&state.pool)
            .await?;
    let mut ranks = [None; 4];
    if metrics.is_some() {
        let circles = if follows_anyone {
            vec![None, Some(user.id)]
        } else {
            vec![None]
        };
        for (i, circle) in circles.into_iter().enumerate() {
            if attempt == 1 && first_eligible {
                ranks[i * 2] = leaderboard::me(&state.pool, Board::First, &scope, circle, user.id)
                    .await?
                    .map(|r| r.rank);
            }
            ranks[i * 2 + 1] = leaderboard::me(&state.pool, Board::Best, &scope, circle, user.id)
                .await?
                .map(|r| r.rank);
        }
    }
    let [rank_first, rank_best, following_first, following_best] = ranks;
    if let Some(r) = &rejected {
        tracing::info!("rejected result {result_id} from {}: {r}", user.login);
    }
    Ok(Json(SubmitResponse {
        result_id,
        attempt: attempt as u32,
        valid: metrics.is_some(),
        rejected: rejected.as_ref().map(Rejected::to_string),
        wpm,
        raw,
        acc,
        consistency,
        rank_first,
        rank_best,
        following_first,
        following_best,
    }))
}

/// Trade a GitHub access token for a ttyp token. The GitHub token is
/// checked once against the GitHub API and never stored or logged.
async fn auth_github(
    State(state): State<AppState>,
    Json(req): Json<AuthRequest>,
) -> ApiResult<AuthResponse> {
    if req.access_token.is_empty() {
        return Err(ApiError::BadRequest("missing access_token".into()));
    }
    let api = state.github_api.clone();
    let user = tokio::task::spawn_blocking(move || auth::github_user(&api, &req.access_token))
        .await
        .map_err(|e| ApiError::Internal(e.into()))??
        .ok_or_else(|| ApiError::BadRequest("github rejected the token".into()))?;
    let (user, token) = auth::issue(&state.pool, user.0, &user.1).await?;
    tracing::info!("login: {}", user.login);
    Ok(Json(AuthResponse {
        token,
        login: user.login,
    }))
}

async fn account(State(state): State<AppState>, headers: HeaderMap) -> ApiResult<Account> {
    let user = require_user(&state, &headers).await?;
    Ok(Json(profile::account(&state.pool, user.id).await?))
}

/// Make the caller's profile public or private.
async fn update_account(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<AccountUpdate>,
) -> ApiResult<Account> {
    let user = require_user(&state, &headers).await?;
    profile::set_public(&state.pool, user.id, req.public).await?;
    Ok(Json(profile::account(&state.pool, user.id).await?))
}

/// A public profile, or the caller's own. Private and unknown users look
/// the same.
async fn user_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(login): Path<String>,
) -> ApiResult<Profile> {
    let viewer = auth::user_from_headers(&state.pool, &headers).await?;
    profile::load(&state.pool, &login, viewer.map(|u| u.id), daily::today())
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound)
}

#[derive(Debug, Deserialize)]
struct PlayerQuery {
    #[serde(default)]
    q: String,
    #[serde(default)]
    offset: u32,
    #[serde(default = "default_limit")]
    limit: u32,
}

/// Public players whose login contains `q`.
async fn search_players(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<PlayerQuery>,
) -> ApiResult<PlayerList> {
    if q.q.chars().count() > 64 {
        return Err(ApiError::BadRequest("search too long".into()));
    }
    let viewer = auth::user_from_headers(&state.pool, &headers).await?;
    let list = players::search(
        &state.pool,
        &q.q,
        viewer.map(|u| u.id),
        q.offset,
        q.limit.clamp(1, MAX_PAGE),
        daily::today(),
    )
    .await?;
    Ok(Json(list))
}

/// Who the caller follows.
async fn follow_list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Vec<PlayerSummary>> {
    let user = require_user(&state, &headers).await?;
    Ok(Json(
        players::list(&state.pool, user.id, daily::today()).await?,
    ))
}

/// Follow or unfollow someone; answers with the new follow list.
async fn update_follow(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<FollowUpdate>,
) -> ApiResult<Vec<PlayerSummary>> {
    let user = require_user(&state, &headers).await?;
    match players::set_follow(&state.pool, user.id, &req.login, req.follow).await? {
        Ok(()) => {}
        Err(FollowError::NotFound) => return Err(ApiError::NotFound),
        Err(FollowError::SelfFollow) => {
            return Err(ApiError::BadRequest("you can't follow yourself".into()));
        }
        Err(FollowError::TooMany) => {
            return Err(ApiError::BadRequest(format!(
                "you already follow {} players",
                players::MAX_FOLLOWS
            )));
        }
    }
    Ok(Json(
        players::list(&state.pool, user.id, daily::today()).await?,
    ))
}

/// Revoke the presented token.
async fn auth_logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<serde_json::Value> {
    let token = auth::bearer(&headers).ok_or(ApiError::Unauthorized)?;
    if !auth::revoke(&state.pool, token).await? {
        return Err(ApiError::Unauthorized);
    }
    Ok(Json(serde_json::json!({})))
}

fn json<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| "[]".into())
}

#[derive(Debug, Deserialize)]
struct LeaderboardQuery {
    #[serde(default = "default_board")]
    board: Board,
    #[serde(default)]
    offset: u32,
    #[serde(default = "default_limit")]
    limit: u32,
    /// `me`: centre the page on the caller's own row.
    around: Option<String>,
    /// Only the caller and who they follow.
    #[serde(default)]
    following: bool,
}

fn default_board() -> Board {
    Board::First
}

fn default_limit() -> u32 {
    50
}

/// The daily boards by daily id; what clients before 2.1.2 call.
async fn leaderboard(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(daily_id): Path<i64>,
    Query(q): Query<LeaderboardQuery>,
) -> ApiResult<Leaderboard> {
    if daily::get(&state.pool, daily_id).await?.is_none() {
        return Err(ApiError::NotFound);
    }
    let page = Paging {
        offset: q.offset,
        limit: q.limit,
        around: q.around,
        following: q.following,
    };
    ranked(&state, &headers, q.board, &Scope::Daily(daily_id), page).await
}

#[derive(Debug, Deserialize)]
struct BoardQuery {
    /// Daily boards: which daily.
    daily: Option<i64>,
    /// All-time boards: the language and mode (`kind` `time`/`words`).
    language: Option<String>,
    kind: Option<String>,
    value: Option<i64>,
    #[serde(default)]
    offset: u32,
    #[serde(default = "default_limit")]
    limit: u32,
    around: Option<String>,
    /// Only the caller and who they follow.
    #[serde(default)]
    following: bool,
}

/// Any board of `boards::BOARDS`, by id, for the target its period needs.
async fn board(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<BoardQuery>,
) -> ApiResult<Leaderboard> {
    let spec = ttyp_core::boards::find(&id).ok_or(ApiError::NotFound)?;
    let scope = match spec.period {
        Period::Daily => {
            let id = q
                .daily
                .ok_or_else(|| ApiError::BadRequest("missing daily".into()))?;
            if daily::get(&state.pool, id).await?.is_none() {
                return Err(ApiError::NotFound);
            }
            Scope::Daily(id)
        }
        Period::AllTime => {
            let (Some(language), Some(kind), Some(value)) = (q.language, q.kind, q.value) else {
                return Err(ApiError::BadRequest(
                    "missing language, kind or value".into(),
                ));
            };
            let mode = daily::mode_from(&kind, value)
                .ok_or_else(|| ApiError::BadRequest("unknown mode".into()))?;
            Scope::Mode { language, mode }
        }
    };
    let page = Paging {
        offset: q.offset,
        limit: q.limit,
        around: q.around,
        following: q.following,
    };
    ranked(&state, &headers, spec.rule, &scope, page).await
}

struct Paging {
    offset: u32,
    limit: u32,
    /// `me`: centre the page on the caller's own row.
    around: Option<String>,
    following: bool,
}

/// A page of one board, with the caller's own row when logged in.
async fn ranked(
    state: &AppState,
    headers: &HeaderMap,
    rule: Board,
    scope: &Scope,
    page: Paging,
) -> ApiResult<Leaderboard> {
    let limit = page.limit.clamp(1, MAX_PAGE);
    let user = auth::user_from_headers(&state.pool, headers).await?;
    let circle = match (&user, page.following) {
        (Some(u), true) => Some(u.id),
        (None, true) => return Err(ApiError::Unauthorized),
        (_, false) => None,
    };
    let me = match &user {
        Some(u) => leaderboard::me(&state.pool, rule, scope, circle, u.id).await?,
        None => None,
    };
    let mut offset = page.offset;
    if page.around.as_deref() == Some("me")
        && let Some(mine) = &me
    {
        offset = mine.rank.saturating_sub(1).saturating_sub(limit / 2);
    }
    let (rows, total) = leaderboard::page(&state.pool, rule, scope, circle, offset, limit).await?;
    Ok(Json(Leaderboard {
        board: rule,
        rows,
        offset,
        total,
        me,
    }))
}

async fn result_by_id(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<ResultDetail> {
    let row = sqlx::query(
        "SELECT r.id, r.attempt, r.wpm, r.raw, r.acc, r.consistency, r.chars, \
                r.wpm_per_second, r.raw_per_second, r.errors_per_second, r.valid, \
                u.github_login, d.id AS daily_id, d.date, d.language, d.mode_kind, d.mode_value \
         FROM results r JOIN users u ON u.id = r.user_id JOIN daily_tests d ON d.id = r.daily_id \
         WHERE r.id = ?1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(ApiError::NotFound)?;
    if row.get::<i64, _>("valid") == 0 {
        return Err(ApiError::NotFound);
    }
    let mode = daily::mode_from(&row.get::<String, _>("mode_kind"), row.get("mode_value"))
        .ok_or(ApiError::NotFound)?;
    let wpm_per_second: Vec<f64> = serde_json::from_str(&row.get::<String, _>("wpm_per_second"))?;
    let raw_per_second: Vec<f64> = serde_json::from_str(&row.get::<String, _>("raw_per_second"))?;
    let errors_per_second: Vec<u32> =
        serde_json::from_str(&row.get::<String, _>("errors_per_second"))?;
    Ok(Json(ResultDetail {
        id,
        daily: DailySummary {
            id: row.get("daily_id"),
            date: row.get("date"),
            language: row.get("language"),
            mode,
        },
        user: row.get("github_login"),
        attempt: row.get::<i64, _>("attempt") as u32,
        wpm: row.get("wpm"),
        raw: row.get("raw"),
        acc: row.get("acc"),
        consistency: row.get("consistency"),
        duration_s: wpm_per_second.len() as f64,
        chars: serde_json::from_str(&row.get::<String, _>("chars"))?,
        wpm_per_second,
        raw_per_second,
        errors_per_second,
    }))
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use axum::body::Body;
    use axum::http::{Request, header};
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    use ttyp_core::test::{FixedGenerator, Metrics, Mode, TestEngine};

    use super::*;

    async fn app() -> (Router, AppState) {
        let pool = crate::db::open_memory().await;
        let state = AppState::new(pool, Arc::new(LanguageRegistry::builtin()));
        (router(state.clone()), state)
    }

    async fn call(app: &Router, req: Request<Body>) -> (StatusCode, serde_json::Value) {
        let res = app.clone().oneshot(req).await.unwrap();
        let status = res.status();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let body = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
        (status, body)
    }

    fn get(path: &str, token: Option<&str>) -> Request<Body> {
        let mut r = Request::get(path);
        if let Some(t) = token {
            r = r.header(header::AUTHORIZATION, format!("Bearer {t}"));
        }
        r.body(Body::empty()).unwrap()
    }

    fn post(path: &str, token: Option<&str>, body: &impl serde::Serialize) -> Request<Body> {
        let mut r = Request::post(path).header(header::CONTENT_TYPE, "application/json");
        if let Some(t) = token {
            r = r.header(header::AUTHORIZATION, format!("Bearer {t}"));
        }
        r.body(Body::from(serde_json::to_vec(body).unwrap()))
            .unwrap()
    }

    /// Type the daily like a client would (recording), return what the
    /// client would show and the log it would send.
    fn type_daily(
        daily: &Daily,
        ms_per_key: u64,
        typo: bool,
    ) -> (Metrics, Vec<ttyp_core::test::KeyEvent>) {
        let mut e = TestEngine::new(
            daily.mode,
            Box::new(FixedGenerator::new(daily.words.clone())),
        );
        e.record_keys();
        let t0 = Instant::now();
        let mut t = t0;
        let mut i = 0u64;
        for (w, word) in daily.words.iter().enumerate() {
            for c in word.chars() {
                if typo && w == 1 {
                    e.type_char_at('x', t0 + Duration::from_millis(ms_per_key * i));
                    i += 1;
                    e.backspace();
                }
                t = t0 + Duration::from_millis(ms_per_key * i);
                e.type_char_at(c, t);
                i += 1;
            }
            e.type_char_at(' ', t0 + Duration::from_millis(ms_per_key * i));
            i += 1;
        }
        while e.status() == ttyp_core::test::Status::Running {
            t += Duration::from_millis(100);
            e.tick(t);
        }
        (Metrics::from_engine(&e), e.keylog().unwrap().to_vec())
    }

    async fn start(app: &Router, token: &str, daily_id: i64) -> StartResponse {
        let (status, body) = call(
            app,
            post(&format!("/dailies/{daily_id}/start"), Some(token), &()),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        serde_json::from_value(body).unwrap()
    }

    /// A stand-in for api.github.com on a local port: one known token.
    async fn mock_github() -> String {
        async fn user(headers: HeaderMap) -> Response {
            match headers
                .get(header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
            {
                Some("Bearer gho_good") => {
                    Json(serde_json::json!({"id": 583231, "login": "octocat", "name": "x"}))
                        .into_response()
                }
                _ => (StatusCode::UNAUTHORIZED, "bad credentials").into_response(),
            }
        }
        let app = Router::new().route("/user", axum::routing::get(user));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn github_login_issues_and_revokes_tokens() {
        let (_, mut state) = app().await;
        state.github_api = mock_github().await;
        let app = router(state.clone());

        let bad = AuthRequest {
            access_token: "gho_bad".into(),
        };
        let (status, body) = call(&app, post("/auth/github", None, &bad)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

        let good = AuthRequest {
            access_token: "gho_good".into(),
        };
        let (status, body) = call(&app, post("/auth/github", None, &good)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let auth: AuthResponse = serde_json::from_value(body).unwrap();
        assert_eq!(auth.login, "octocat");
        assert_eq!(auth.token.len(), 43);
        // Only the hash is stored, and no GitHub token anywhere.
        let stored: Vec<Vec<u8>> = sqlx::query_scalar("SELECT token_hash FROM api_tokens")
            .fetch_all(&state.pool)
            .await
            .unwrap();
        assert_eq!(stored, vec![auth::hash(&auth.token)]);

        // The token authenticates, then logout kills it.
        let (_, body) = call(&app, get("/dailies/today", None)).await;
        let list: Vec<DailySummary> = serde_json::from_value(body).unwrap();
        let req = SubmitRequest {
            daily_id: list[0].id,
            keylog: vec![],
            start_id: None,
        };
        let (status, _) = call(&app, post("/results", Some(&auth.token), &req)).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "empty log is stored as invalid, not refused"
        );
        let (status, _) = call(&app, post("/auth/logout", Some(&auth.token), &())).await;
        assert_eq!(status, StatusCode::OK);
        let (status, _) = call(&app, post("/results", Some(&auth.token), &req)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let (status, _) = call(&app, post("/auth/logout", Some(&auth.token), &())).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn dailies_are_listed_and_fetched() {
        let (app, _) = app().await;
        let (status, body) = call(&app, get("/health", None)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["dailies_today"], 0);
        let (status, body) = call(&app, get("/dailies/today", None)).await;
        assert_eq!(status, StatusCode::OK);
        let list: Vec<DailySummary> = serde_json::from_value(body).unwrap();
        assert_eq!(list.len(), 14);
        let (status, body) = call(&app, get(&format!("/dailies/{}", list[0].id), None)).await;
        assert_eq!(status, StatusCode::OK);
        let d: Daily = serde_json::from_value(body).unwrap();
        assert_eq!(d.words.len(), daily::words_for(d.mode));
        let (status, _) = call(&app, get("/dailies/9999", None)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (_, body) = call(&app, get("/health", None)).await;
        assert_eq!(body["dailies_today"], 14);
        // By date: today lists the same; another day is empty; junk is 400.
        let today = daily::today().to_string();
        let (status, body) = call(&app, get(&format!("/dailies?date={today}"), None)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.as_array().map(Vec::len), Some(14));
        let (_, body) = call(&app, get("/dailies?date=2000-01-01", None)).await;
        assert_eq!(body.as_array().map(Vec::len), Some(0));
        let (status, _) = call(&app, get("/dailies?date=yesterday", None)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn submission_is_replayed_and_ranked() {
        let (app, state) = app().await;
        let (_, body) = call(&app, get("/dailies/today", None)).await;
        let list: Vec<DailySummary> = serde_json::from_value(body).unwrap();
        let words10 = list
            .iter()
            .find(|d| d.mode == Mode::Words(10) && d.language == "english")
            .unwrap();
        let (_, body) = call(&app, get(&format!("/dailies/{}", words10.id), None)).await;
        let daily: Daily = serde_json::from_value(body).unwrap();

        let (_, alice) = auth::issue(&state.pool, 1, "alice").await.unwrap();
        let (_, bob) = auth::issue(&state.pool, 2, "bob").await.unwrap();

        // No token: refused.
        let (shown, keylog) = type_daily(&daily, 120, true);
        let (status, _) = call(
            &app,
            post(&format!("/dailies/{}/start", daily.id), None, &()),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let start_alice = start(&app, &alice, daily.id).await;
        assert_eq!(start_alice.attempt, 1);
        let req = SubmitRequest {
            daily_id: daily.id,
            keylog: keylog.clone(),
            start_id: Some(start_alice.start_id),
        };
        let (status, _) = call(&app, post("/results", None, &req)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        // Alice, first try: server metrics equal what the client showed.
        let (status, body) = call(&app, post("/results", Some(&alice), &req)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let r: SubmitResponse = serde_json::from_value(body).unwrap();
        assert!(r.valid);
        assert_eq!(
            (r.wpm, r.raw, r.acc, r.consistency),
            (shown.wpm, shown.raw, shown.accuracy, shown.consistency)
        );
        assert_eq!(
            (r.attempt, r.rank_first, r.rank_best),
            (1, Some(1), Some(1))
        );

        // Bob types faster: takes both #1 spots.
        let (_, fast) = type_daily(&daily, 80, false);
        let start_bob = start(&app, &bob, daily.id).await;
        let (_, body) = call(
            &app,
            post(
                "/results",
                Some(&bob),
                &SubmitRequest {
                    daily_id: daily.id,
                    keylog: fast,
                    start_id: Some(start_bob.start_id),
                },
            ),
        )
        .await;
        let r: SubmitResponse = serde_json::from_value(body).unwrap();
        assert_eq!((r.rank_first, r.rank_best), (Some(1), Some(1)));

        // Alice's second, faster run: no first-try rank, but best moves up.
        let (_, faster) = type_daily(&daily, 60, false);
        let (_, body) = call(
            &app,
            post(
                "/results",
                Some(&alice),
                &SubmitRequest {
                    daily_id: daily.id,
                    keylog: faster,
                    start_id: Some(start(&app, &alice, daily.id).await.start_id),
                },
            ),
        )
        .await;
        let r: SubmitResponse = serde_json::from_value(body).unwrap();
        assert_eq!((r.attempt, r.rank_first, r.rank_best), (2, None, Some(1)));
        let alice_best = r.result_id;

        // A doctored log is stored but invalid and unranked.
        let (_, body) = call(
            &app,
            post(
                "/results",
                Some(&bob),
                &SubmitRequest {
                    daily_id: daily.id,
                    keylog: keylog[..3].to_vec(),
                    start_id: None,
                },
            ),
        )
        .await;
        let r: SubmitResponse = serde_json::from_value(body).unwrap();
        assert!(!r.valid);
        assert_eq!(r.rejected.as_deref(), Some("test did not finish"));
        assert_eq!((r.attempt, r.rank_best), (2, None));

        // Boards.
        let (status, body) = call(
            &app,
            get(
                &format!("/leaderboard/{}?board=first", daily.id),
                Some(&alice),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let lb: Leaderboard = serde_json::from_value(body).unwrap();
        let names: Vec<&str> = lb.rows.iter().map(|r| r.user.as_str()).collect();
        assert_eq!(names, ["bob", "alice"]);
        assert_eq!(lb.me.unwrap().rank, 2);
        assert_eq!(lb.total, 2);
        let (_, body) = call(
            &app,
            get(
                &format!("/leaderboard/{}?board=best&limit=1", daily.id),
                None,
            ),
        )
        .await;
        let lb: Leaderboard = serde_json::from_value(body).unwrap();
        assert_eq!(lb.rows[0].user, "alice");
        assert_eq!(lb.rows[0].result_id, alice_best);
        assert!(lb.me.is_none());
        let (status, _) = call(&app, get("/leaderboard/9999", None)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        // The same boards through `/boards`, and the all-time ones.
        use ttyp_core::boards::Target;
        let daily_target = Target::Daily(daily.id).query();
        let (_, body) = call(
            &app,
            get(&format!("/boards/daily-first?{daily_target}"), Some(&alice)),
        )
        .await;
        let lb: Leaderboard = serde_json::from_value(body).unwrap();
        let names: Vec<&str> = lb.rows.iter().map(|r| r.user.as_str()).collect();
        assert_eq!(names, ["bob", "alice"]);
        let all_time = Target::Mode {
            language: daily.language.clone(),
            mode: daily.mode,
        }
        .query();
        let (status, body) = call(
            &app,
            get(&format!("/boards/alltime-best?{all_time}"), Some(&bob)),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let lb: Leaderboard = serde_json::from_value(body).unwrap();
        assert_eq!(lb.rows[0].result_id, alice_best);
        assert_eq!(lb.rows[0].date.as_deref(), Some(daily.date.as_str()));
        assert_eq!(lb.me.unwrap().rank, 2);
        for (path, want) in [
            ("/boards/nope?daily=1".to_string(), StatusCode::NOT_FOUND),
            ("/boards/daily-best".to_string(), StatusCode::BAD_REQUEST),
            (
                "/boards/alltime-best?daily=1".to_string(),
                StatusCode::BAD_REQUEST,
            ),
            (
                "/boards/alltime-best?language=english&kind=laps&value=3".to_string(),
                StatusCode::BAD_REQUEST,
            ),
        ] {
            let (status, _) = call(&app, get(&path, None)).await;
            assert_eq!(status, want, "{path}");
        }

        // The graph view of a run.
        let (status, body) = call(&app, get(&format!("/results/{alice_best}"), None)).await;
        assert_eq!(status, StatusCode::OK);
        let d: ResultDetail = serde_json::from_value(body).unwrap();
        assert_eq!(d.user, "alice");
        assert_eq!(d.daily.id, daily.id);
        assert_eq!(d.wpm_per_second.len(), d.raw_per_second.len());
        assert!(!d.wpm_per_second.is_empty());
    }

    #[tokio::test]
    async fn restarting_cannot_hide_a_first_try() {
        let (app, state) = app().await;
        let (_, body) = call(&app, get("/dailies/today", None)).await;
        let list: Vec<DailySummary> = serde_json::from_value(body).unwrap();
        let (_, body) = call(&app, get(&format!("/dailies/{}", list[0].id), None)).await;
        let daily: Daily = serde_json::from_value(body).unwrap();
        let (_, alice) = auth::issue(&state.pool, 1, "alice").await.unwrap();
        let (_, bob) = auth::issue(&state.pool, 2, "bob").await.unwrap();
        let (_, carol) = auth::issue(&state.pool, 3, "carol").await.unwrap();
        let submit = |token: String, start_id: Option<i64>| {
            let app = app.clone();
            let (_, keylog) = type_daily(&daily, 90, false);
            let req = SubmitRequest {
                daily_id: daily.id,
                keylog,
                start_id,
            };
            async move { call(&app, post("/results", Some(&token), &req)).await }
        };

        // Alice starts, restarts (nothing submitted), then finishes a run:
        // that run is attempt 2 and not on the first-try board.
        let abandoned = start(&app, &alice, daily.id).await;
        assert_eq!(abandoned.attempt, 1);
        let second = start(&app, &alice, daily.id).await;
        assert_eq!(second.attempt, 2);
        let (status, body) = submit(alice.clone(), Some(second.start_id)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let r: SubmitResponse = serde_json::from_value(body).unwrap();
        assert_eq!((r.attempt, r.rank_first), (2, None));
        assert!(r.rank_best.is_some(), "still on the best board");

        // A start is used once, and only by its owner.
        let (status, _) = submit(alice.clone(), Some(second.start_id)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let (status, _) = submit(bob.clone(), Some(abandoned.start_id)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        // No start (logged out while typing, or an old client): counted,
        // but never a first try.
        let (_, body) = submit(bob.clone(), None).await;
        let r: SubmitResponse = serde_json::from_value(body).unwrap();
        assert_eq!((r.attempt, r.rank_first), (1, None));
        // Starts and start-less runs share one sequence.
        assert_eq!(start(&app, &bob, daily.id).await.attempt, 2);

        // Carol does it properly: her first start is her first try.
        let s = start(&app, &carol, daily.id).await;
        let (_, body) = submit(carol.clone(), Some(s.start_id)).await;
        let r: SubmitResponse = serde_json::from_value(body).unwrap();
        assert_eq!((r.attempt, r.rank_first), (1, Some(1)));
        let (_, body) = call(
            &app,
            get(&format!("/leaderboard/{}?board=first", daily.id), None),
        )
        .await;
        let lb: Leaderboard = serde_json::from_value(body).unwrap();
        let names: Vec<&str> = lb.rows.iter().map(|r| r.user.as_str()).collect();
        assert_eq!(names, ["carol"]);
    }

    #[tokio::test]
    async fn profiles_are_public_until_opted_out() {
        let (app, state) = app().await;
        let (_, body) = call(&app, get("/dailies/today", None)).await;
        let list: Vec<DailySummary> = serde_json::from_value(body).unwrap();
        let (_, body) = call(&app, get(&format!("/dailies/{}", list[0].id), None)).await;
        let daily: Daily = serde_json::from_value(body).unwrap();
        let (_, alice) = auth::issue(&state.pool, 1, "Alice").await.unwrap();
        let (_, bob) = auth::issue(&state.pool, 2, "bob").await.unwrap();
        for ms in [120, 90] {
            let s = start(&app, &alice, daily.id).await;
            let (_, keylog) = type_daily(&daily, ms, false);
            let req = SubmitRequest {
                daily_id: daily.id,
                keylog,
                start_id: Some(s.start_id),
            };
            let (status, _) = call(&app, post("/results", Some(&alice), &req)).await;
            assert_eq!(status, StatusCode::OK);
        }

        // Public by default: anyone can look, login case doesn't matter.
        let (_, body) = call(&app, get("/account", Some(&alice))).await;
        let acct: Account = serde_json::from_value(body).unwrap();
        assert_eq!((acct.login.as_str(), acct.public), ("Alice", true));
        let (status, _) = call(&app, get("/account", None)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let (status, body) = call(&app, get("/users/ALICE", None)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let p: Profile = serde_json::from_value(body).unwrap();
        assert_eq!((p.login.as_str(), p.public), ("Alice", true));
        assert_eq!((p.streak, p.dailies), (1, 1));
        assert_eq!(p.recent.len(), 2);
        assert_eq!(p.recent[0].attempt, 2, "newest first");
        assert_eq!(p.bests.len(), 1, "one per language and mode");
        assert_eq!(p.bests[0].attempt, 2, "the faster run");
        assert!(p.bests[0].wpm > p.recent[1].wpm);

        // Opted out: hidden from others, still visible to its owner.
        let (_, body) = call(
            &app,
            post("/account", Some(&alice), &AccountUpdate { public: false }),
        )
        .await;
        let acct: Account = serde_json::from_value(body).unwrap();
        assert!(!acct.public);
        let (status, _) = call(&app, get("/users/alice", None)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, _) = call(&app, get("/users/alice", Some(&bob))).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, body) = call(&app, get("/users/alice", Some(&alice))).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        // Logging in again keeps the choice.
        auth::issue(&state.pool, 1, "Alice").await.unwrap();
        let (status, _) = call(&app, get("/users/alice", None)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, _) = call(&app, get("/users/nobody", None)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn players_are_searched_and_followed() {
        let (app, state) = app().await;
        let (_, body) = call(&app, get("/dailies/today", None)).await;
        let list: Vec<DailySummary> = serde_json::from_value(body).unwrap();
        let t30 = list
            .iter()
            .find(|d| d.language == "english" && d.mode == Mode::Time(30))
            .unwrap();
        let (_, body) = call(&app, get(&format!("/dailies/{}", t30.id), None)).await;
        let daily: Daily = serde_json::from_value(body).unwrap();
        let (_, ann) = auth::issue(&state.pool, 1, "ann").await.unwrap();
        let (_, anna) = auth::issue(&state.pool, 2, "Anna").await.unwrap();
        let (_, joanne) = auth::issue(&state.pool, 3, "joanne").await.unwrap();
        auth::issue(&state.pool, 4, "bo").await.unwrap();
        let s = start(&app, &anna, daily.id).await;
        let (shown, keylog) = type_daily(&daily, 120, false);
        let req = SubmitRequest {
            daily_id: daily.id,
            keylog,
            start_id: Some(s.start_id),
        };
        let (status, _) = call(&app, post("/results", Some(&anna), &req)).await;
        assert_eq!(status, StatusCode::OK);

        // Exact match, then prefix, then anywhere; any case.
        let (status, body) = call(&app, get("/users?q=ANN", None)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let found: PlayerList = serde_json::from_value(body).unwrap();
        let names: Vec<&str> = found.rows.iter().map(|r| r.login.as_str()).collect();
        assert_eq!(names, ["ann", "Anna", "joanne"]);
        assert_eq!(found.total, 3);
        assert_eq!(found.rows[1].bests, [None, Some(shown.wpm), None]);
        assert!(found.rows.iter().all(|r| r.public && !r.following));
        // Wildcards are literal; paging works.
        let (_, body) = call(&app, get("/users?q=%25", None)).await;
        let none: PlayerList = serde_json::from_value(body).unwrap();
        assert_eq!(none.total, 0);
        let (_, body) = call(&app, get("/users?offset=1&limit=2", None)).await;
        let page: PlayerList = serde_json::from_value(body).unwrap();
        assert_eq!((page.total, page.rows.len()), (4, 2));
        assert_eq!(page.rows[0].login, "Anna");

        // Following needs a login and someone else's public profile.
        let follow = |login: &str, follow| FollowUpdate {
            login: login.into(),
            follow,
        };
        let (status, _) = call(&app, post("/follows", None, &follow("anna", true))).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let (status, _) = call(&app, post("/follows", Some(&ann), &follow("ann", true))).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let (status, _) = call(&app, post("/follows", Some(&ann), &follow("nobody", true))).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        call(&app, post("/follows", Some(&ann), &follow("anna", true))).await;
        call(&app, post("/follows", Some(&ann), &follow("JOANNE", true))).await;
        let (status, body) = call(&app, post("/follows", Some(&ann), &follow("anna", true))).await;
        assert_eq!(status, StatusCode::OK, "following twice is fine");
        let mine: Vec<PlayerSummary> = serde_json::from_value(body).unwrap();
        let names: Vec<&str> = mine.iter().map(|r| r.login.as_str()).collect();
        assert_eq!(names, ["joanne", "Anna"], "newest follow first");
        assert!(mine.iter().all(|r| r.following));
        // One way: anna doesn't follow ann back.
        let (_, body) = call(&app, get("/follows", Some(&anna))).await;
        assert_eq!(body, serde_json::json!([]));
        let (_, body) = call(&app, get("/users?q=ann", Some(&ann))).await;
        let found: PlayerList = serde_json::from_value(body).unwrap();
        let flags: Vec<bool> = found.rows.iter().map(|r| r.following).collect();
        assert_eq!(flags, [false, true, true]);
        let (_, body) = call(&app, get("/users/anna", Some(&ann))).await;
        assert_eq!(body["following"], true);
        let (_, body) = call(&app, get("/users/anna", None)).await;
        assert_eq!(body["following"], false);

        // Boards cut down to ann and who she follows; ranks start at 1.
        let url = format!("/boards/daily-best?daily={}&following=true", daily.id);
        let (status, _) = call(&app, get(&url, None)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let s = start(&app, &ann, daily.id).await;
        let (_, keylog) = type_daily(&daily, 200, false);
        let req = SubmitRequest {
            daily_id: daily.id,
            keylog,
            start_id: Some(s.start_id),
        };
        let (_, body) = call(&app, post("/results", Some(&ann), &req)).await;
        let r: SubmitResponse = serde_json::from_value(body).unwrap();
        assert_eq!((r.rank_best, r.following_best), (Some(2), Some(2)));
        assert_eq!(r.following_first, Some(2));
        let (status, body) = call(&app, get(&url, Some(&ann))).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let lb: Leaderboard = serde_json::from_value(body).unwrap();
        let names: Vec<&str> = lb.rows.iter().map(|r| r.user.as_str()).collect();
        assert_eq!(names, ["Anna", "ann"]);
        assert_eq!(lb.me.map(|m| m.rank), Some(2));
        // Anna follows nobody: her submission has no following ranks.
        let s = start(&app, &anna, daily.id).await;
        let (_, keylog) = type_daily(&daily, 120, false);
        let req = SubmitRequest {
            daily_id: daily.id,
            keylog,
            start_id: Some(s.start_id),
        };
        let (_, body) = call(&app, post("/results", Some(&anna), &req)).await;
        assert!(body.get("following_best").is_none(), "{body}");

        // Going private hides joanne from search and blanks her row, but
        // she can still be unfollowed; she can't be followed anew.
        call(
            &app,
            post("/account", Some(&joanne), &AccountUpdate { public: false }),
        )
        .await;
        let (_, body) = call(&app, get("/users?q=ann", None)).await;
        let found: PlayerList = serde_json::from_value(body).unwrap();
        assert_eq!(found.total, 2);
        let (_, body) = call(&app, get("/follows", Some(&ann))).await;
        let mine: Vec<PlayerSummary> = serde_json::from_value(body).unwrap();
        assert!(!mine[0].public);
        let (status, body) =
            call(&app, post("/follows", Some(&ann), &follow("joanne", false))).await;
        assert_eq!(status, StatusCode::OK);
        let mine: Vec<PlayerSummary> = serde_json::from_value(body).unwrap();
        assert_eq!(mine.len(), 1);
        let (status, _) = call(&app, post("/follows", Some(&ann), &follow("joanne", true))).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}
