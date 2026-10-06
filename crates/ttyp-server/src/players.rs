//! Finding players and following them: `GET /users?q=` searches public
//! profiles, `GET`/`POST /follows` is the caller's one-way follow list.
//! Every list row is a `PlayerSummary`, the profile's headline numbers.

use std::collections::HashSet;

use anyhow::Result;
use chrono::NaiveDate;
use sqlx::Row;
use sqlx::sqlite::SqlitePool;
use ttyp_core::api::{PlayerList, PlayerSummary};

use crate::db;
use crate::profile::{badges_of, id_list, main_bests};

/// Most players one account may follow.
pub const MAX_FOLLOWS: i64 = 1000;

/// Why a follow change was refused.
#[derive(Debug, PartialEq, Eq)]
pub enum FollowError {
    /// No such user, or a private one (they look the same).
    NotFound,
    SelfFollow,
    TooMany,
}

/// Public players whose login contains `query` (any case): an exact match
/// first, then logins starting with it, then the rest, each alphabetical.
pub async fn search(
    pool: &SqlitePool,
    query: &str,
    viewer: Option<i64>,
    offset: u32,
    limit: u32,
    today: NaiveDate,
) -> Result<PlayerList> {
    let query = query.trim();
    let escaped = escape_like(query);
    let contains = format!("%{escaped}%");
    let prefix = format!("{escaped}%");
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM users WHERE public = 1 AND github_login LIKE ?1 ESCAPE '\\'",
    )
    .bind(&contains)
    .fetch_one(pool)
    .await?;
    let rows = sqlx::query(
        "SELECT id, github_login FROM users \
         WHERE public = 1 AND github_login LIKE ?1 ESCAPE '\\' \
         ORDER BY github_login = ?2 COLLATE NOCASE DESC, \
                  github_login LIKE ?3 ESCAPE '\\' DESC, \
                  github_login COLLATE NOCASE, id \
         LIMIT ?4 OFFSET ?5",
    )
    .bind(&contains)
    .bind(query)
    .bind(&prefix)
    .bind(i64::from(limit))
    .bind(i64::from(offset))
    .fetch_all(pool)
    .await?;
    let users: Vec<(i64, String, bool)> = rows
        .iter()
        .map(|r| (r.get("id"), r.get("github_login"), true))
        .collect();
    Ok(PlayerList {
        rows: summaries(pool, &users, viewer, today).await?,
        offset,
        total: total as u32,
    })
}

/// Who `viewer` follows, most recently followed first.
pub async fn list(pool: &SqlitePool, viewer: i64, today: NaiveDate) -> Result<Vec<PlayerSummary>> {
    let rows = sqlx::query(
        "SELECT u.id, u.github_login, u.public FROM follows f \
         JOIN users u ON u.id = f.followee_id \
         WHERE f.follower_id = ?1 ORDER BY f.created_at DESC, u.id DESC",
    )
    .bind(viewer)
    .fetch_all(pool)
    .await?;
    let users: Vec<(i64, String, bool)> = rows
        .iter()
        .map(|r| {
            (
                r.get("id"),
                r.get("github_login"),
                r.get::<i64, _>("public") != 0,
            )
        })
        .collect();
    summaries(pool, &users, Some(viewer), today).await
}

/// Whether `viewer` follows `user`.
pub async fn follows(pool: &SqlitePool, viewer: i64, user: i64) -> Result<bool> {
    let n: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM follows WHERE follower_id = ?1 AND followee_id = ?2",
    )
    .bind(viewer)
    .bind(user)
    .fetch_one(pool)
    .await?;
    Ok(n > 0)
}

/// Follow or unfollow `login`. Following needs a public profile; unfollowing
/// works whatever the profile has become. Both are idempotent.
pub async fn set_follow(
    pool: &SqlitePool,
    viewer: i64,
    login: &str,
    follow: bool,
) -> Result<Result<(), FollowError>> {
    let Some(user) =
        sqlx::query("SELECT id, public FROM users WHERE github_login = ?1 COLLATE NOCASE")
            .bind(login)
            .fetch_optional(pool)
            .await?
    else {
        return Ok(Err(FollowError::NotFound));
    };
    let id: i64 = user.get("id");
    if !follow {
        sqlx::query("DELETE FROM follows WHERE follower_id = ?1 AND followee_id = ?2")
            .bind(viewer)
            .bind(id)
            .execute(pool)
            .await?;
        return Ok(Ok(()));
    }
    if id == viewer {
        return Ok(Err(FollowError::SelfFollow));
    }
    if user.get::<i64, _>("public") == 0 {
        return Ok(Err(FollowError::NotFound));
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM follows WHERE follower_id = ?1")
        .bind(viewer)
        .fetch_one(pool)
        .await?;
    if count >= MAX_FOLLOWS && !follows(pool, viewer, id).await? {
        return Ok(Err(FollowError::TooMany));
    }
    sqlx::query(
        "INSERT INTO follows (follower_id, followee_id, created_at) VALUES (?1, ?2, ?3) \
         ON CONFLICT DO NOTHING",
    )
    .bind(viewer)
    .bind(id)
    .bind(db::now())
    .execute(pool)
    .await?;
    Ok(Ok(()))
}

/// Summaries of `users` (id, login, public), in their order. Private
/// players get no numbers.
async fn summaries(
    pool: &SqlitePool,
    users: &[(i64, String, bool)],
    viewer: Option<i64>,
    today: NaiveDate,
) -> Result<Vec<PlayerSummary>> {
    let ids: Vec<i64> = users
        .iter()
        .filter(|(_, _, public)| *public)
        .map(|(id, _, _)| *id)
        .collect();
    let mut bests = main_bests(pool, &ids).await?;
    let mut badges = badges_of(pool, &ids, today).await?;
    let followed: HashSet<i64> = match viewer {
        Some(v) => sqlx::query_scalar(
            "SELECT followee_id FROM follows \
             WHERE follower_id = ?1 AND followee_id IN (SELECT value FROM json_each(?2))",
        )
        .bind(v)
        .bind(id_list(&users.iter().map(|u| u.0).collect::<Vec<_>>()))
        .fetch_all(pool)
        .await?
        .into_iter()
        .collect(),
        None => HashSet::new(),
    };
    Ok(users
        .iter()
        .map(|(id, login, public)| PlayerSummary {
            login: login.clone(),
            bests: bests.remove(id).unwrap_or_default(),
            badges: badges.remove(id).unwrap_or_default(),
            following: followed.contains(id),
            public: *public,
        })
        .collect())
}

/// `s` with LIKE's wildcards and the escape character escaped by `\`.
fn escape_like(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '%' | '_' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}
