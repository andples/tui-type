//! Every board in `ttyp_core::boards::BOARDS` comes from one query: the
//! runs in a `Scope` (one daily, or every daily of a language and mode),
//! cut down by the board's rule to one run per player, ranked. *First try*
//! keeps attempt 1 (when the server saw it start, see `api::start`); *best*
//! keeps every valid run. Each player's highest run is kept, and the board
//! orders by wpm, then accuracy, then who got there first.

use anyhow::Result;
use sqlx::sqlite::{Sqlite, SqliteArguments, SqlitePool, SqliteRow};
use sqlx::{AssertSqlSafe, Row};
use ttyp_core::api::{Board, LeaderboardRow};
use ttyp_core::test::Mode;

/// Which `results r` rows are on the first-try board (plus `r.valid = 1`).
pub const FIRST_TRY: &str = "r.attempt = 1 AND r.first_eligible = 1";
/// How every board orders its rows.
pub const RANK_ORDER: &str = "r.wpm DESC, r.acc DESC, r.created_at ASC";

/// The runs a board ranks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    Daily(i64),
    /// Every daily of a language and mode.
    Mode {
        language: String,
        mode: Mode,
    },
}

impl Scope {
    /// The `daily_tests d` filter, using parameters `?1`..`?{params}`.
    fn filter(&self) -> &'static str {
        match self {
            Scope::Daily(_) => "d.id = ?1",
            Scope::Mode { .. } => "d.language = ?1 AND d.mode_kind = ?2 AND d.mode_value = ?3",
        }
    }

    fn params(&self) -> usize {
        match self {
            Scope::Daily(_) => 1,
            Scope::Mode { .. } => 3,
        }
    }

    fn bind<'q>(
        &self,
        q: sqlx::query::Query<'q, Sqlite, SqliteArguments>,
    ) -> sqlx::query::Query<'q, Sqlite, SqliteArguments> {
        match self {
            Scope::Daily(id) => q.bind(*id),
            Scope::Mode { language, mode } => q
                .bind(language.clone())
                .bind(mode.kind())
                .bind(i64::from(mode.value())),
        }
    }
}

/// The ranked rows of one board as a common table expression `board`.
fn board_cte(rule: Board, scope: &Scope) -> String {
    let rule = match rule {
        Board::First => FIRST_TRY,
        Board::Best => "1",
    };
    let scope = scope.filter();
    format!(
        "WITH runs AS (\
           SELECT r.*, d.date, \
                  ROW_NUMBER() OVER (PARTITION BY r.user_id ORDER BY {RANK_ORDER}) AS n \
           FROM results r JOIN daily_tests d ON d.id = r.daily_id \
           WHERE {scope} AND r.valid = 1 AND {rule}), \
         board AS (\
           SELECT r.id AS result_id, r.user_id, u.github_login AS user, \
                  r.wpm, r.raw, r.acc, r.consistency, r.date, \
                  ROW_NUMBER() OVER (ORDER BY {RANK_ORDER}) AS rank \
           FROM runs r JOIN users u ON u.id = r.user_id WHERE r.n = 1) "
    )
}

fn row_from(row: &SqliteRow) -> LeaderboardRow {
    LeaderboardRow {
        rank: row.get::<i64, _>("rank") as u32,
        user: row.get("user"),
        wpm: row.get("wpm"),
        raw: row.get("raw"),
        acc: row.get("acc"),
        consistency: row.get("consistency"),
        result_id: row.get("result_id"),
        date: Some(row.get("date")),
    }
}

/// `limit` rows from `offset`, plus the board's size.
pub async fn page(
    pool: &SqlitePool,
    rule: Board,
    scope: &Scope,
    offset: u32,
    limit: u32,
) -> Result<(Vec<LeaderboardRow>, u32)> {
    let n = scope.params();
    let sql = board_cte(rule, scope)
        + &format!(
            "SELECT *, (SELECT count(*) FROM board) AS total FROM board \
             ORDER BY rank LIMIT ?{} OFFSET ?{}",
            n + 1,
            n + 2
        );
    let rows = scope
        .bind(sqlx::query(AssertSqlSafe(sql)))
        .bind(i64::from(limit))
        .bind(i64::from(offset))
        .fetch_all(pool)
        .await?;
    let total = match rows.first() {
        Some(r) => r.get::<i64, _>("total") as u32,
        // Past the end: count separately.
        None => {
            let sql = board_cte(rule, scope) + "SELECT count(*) FROM board";
            scope
                .bind(sqlx::query(AssertSqlSafe(sql)))
                .fetch_one(pool)
                .await?
                .get::<i64, _>(0) as u32
        }
    };
    Ok((rows.iter().map(row_from).collect(), total))
}

/// One user's row on a board.
pub async fn me(
    pool: &SqlitePool,
    rule: Board,
    scope: &Scope,
    user_id: i64,
) -> Result<Option<LeaderboardRow>> {
    let sql = board_cte(rule, scope)
        + &format!(
            "SELECT * FROM board WHERE user_id = ?{}",
            scope.params() + 1
        );
    let row = scope
        .bind(sqlx::query(AssertSqlSafe(sql)))
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.as_ref().map(row_from))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Insert a valid result row directly (no replay).
    pub async fn seed(
        pool: &SqlitePool,
        user_id: i64,
        daily_id: i64,
        attempt: i64,
        wpm: f64,
        acc: f64,
    ) -> i64 {
        let done = sqlx::query(
            "INSERT INTO results (user_id, daily_id, attempt, wpm, raw, acc, consistency, chars, \
             wpm_per_second, raw_per_second, errors_per_second, keylog, valid, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?4, ?5, 80, '{}', '[]', '[]', '[]', x'', 1, ?6)",
        )
        .bind(user_id)
        .bind(daily_id)
        .bind(attempt)
        .bind(wpm)
        .bind(acc)
        .bind(format!("2026-09-29T00:00:{attempt:02}Z"))
        .execute(pool)
        .await
        .unwrap();
        done.last_insert_rowid()
    }

    pub async fn user(pool: &SqlitePool, login: &str) -> i64 {
        let (u, _) = crate::auth::issue(
            pool,
            login.len() as i64 * 1000 + login.bytes().map(i64::from).sum::<i64>(),
            login,
        )
        .await
        .unwrap();
        u.id
    }

    pub async fn daily(pool: &SqlitePool) -> i64 {
        daily_on(pool, 29, Mode::Words(10)).await
    }

    /// A daily on 2026-09-`day`.
    pub async fn daily_on(pool: &SqlitePool, day: u32, mode: Mode) -> i64 {
        let langs = ttyp_core::language::LanguageRegistry::builtin();
        let date = chrono::NaiveDate::from_ymd_opt(2026, 9, day).unwrap();
        crate::daily::insert(
            &pool.clone(),
            &langs,
            date,
            "english",
            mode,
            "schedule",
            None,
        )
        .await
        .unwrap()
        .unwrap()
    }

    #[tokio::test]
    async fn first_and_best_rank_differently() {
        let pool = crate::db::open_memory().await;
        let d = daily(&pool).await;
        let a = user(&pool, "sprinter").await;
        let b = user(&pool, "quietkeys").await;
        let c = user(&pool, "andples").await;
        seed(&pool, a, d, 1, 142.0, 98.1).await;
        seed(&pool, b, d, 1, 138.0, 99.0).await;
        seed(&pool, b, d, 2, 151.0, 99.0).await;
        seed(&pool, c, d, 1, 131.0, 97.4).await;
        seed(&pool, c, d, 2, 100.0, 90.0).await;
        // Equal wpm: higher accuracy wins, then the earlier run.
        let e = user(&pool, "tie").await;
        seed(&pool, e, d, 1, 142.0, 99.5).await;

        let (first, total) = page(&pool, Board::First, &Scope::Daily(d), 0, 50)
            .await
            .unwrap();
        assert_eq!(total, 4);
        let names: Vec<&str> = first.iter().map(|r| r.user.as_str()).collect();
        assert_eq!(names, ["tie", "sprinter", "quietkeys", "andples"]);
        assert_eq!(first[1].rank, 2);

        let (best, total) = page(&pool, Board::Best, &Scope::Daily(d), 0, 50)
            .await
            .unwrap();
        assert_eq!(total, 4);
        let names: Vec<&str> = best.iter().map(|r| r.user.as_str()).collect();
        assert_eq!(names, ["quietkeys", "tie", "sprinter", "andples"]);
        assert_eq!(best[0].wpm, 151.0);
        assert_eq!(best[3].wpm, 131.0, "best, not latest");

        let (page2, total) = page(&pool, Board::Best, &Scope::Daily(d), 3, 2)
            .await
            .unwrap();
        assert_eq!((page2.len(), total), (1, 4));
        assert_eq!(page2[0].rank, 4);
        let (empty, total) = page(&pool, Board::Best, &Scope::Daily(d), 10, 2)
            .await
            .unwrap();
        assert_eq!((empty.len(), total), (0, 4));

        let mine = me(&pool, Board::Best, &Scope::Daily(d), c)
            .await
            .unwrap()
            .unwrap();
        assert_eq!((mine.rank, mine.wpm), (4, 131.0));
        assert_eq!(
            me(&pool, Board::First, &Scope::Daily(d), 999)
                .await
                .unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn all_time_keeps_each_players_best_across_dailies() {
        let pool = crate::db::open_memory().await;
        let d1 = daily_on(&pool, 27, Mode::Time(30)).await;
        let d2 = daily_on(&pool, 28, Mode::Time(30)).await;
        let other = daily_on(&pool, 28, Mode::Time(15)).await;
        let a = user(&pool, "sprinter").await;
        let b = user(&pool, "quietkeys").await;
        seed(&pool, a, d1, 1, 120.0, 98.0).await;
        seed(&pool, a, d2, 1, 110.0, 98.0).await;
        seed(&pool, a, d2, 2, 140.0, 97.0).await;
        seed(&pool, b, d2, 1, 130.0, 99.0).await;
        seed(&pool, b, other, 1, 200.0, 99.0).await;

        let scope = Scope::Mode {
            language: "english".into(),
            mode: Mode::Time(30),
        };
        let (best, total) = page(&pool, Board::Best, &scope, 0, 50).await.unwrap();
        assert_eq!(total, 2, "one row per player, other modes left out");
        let got: Vec<(&str, f64)> = best.iter().map(|r| (r.user.as_str(), r.wpm)).collect();
        assert_eq!(got, [("sprinter", 140.0), ("quietkeys", 130.0)]);
        assert_eq!(best[0].date.as_deref(), Some("2026-09-28"));

        let (first, _) = page(&pool, Board::First, &scope, 0, 50).await.unwrap();
        let got: Vec<(&str, f64)> = first.iter().map(|r| (r.user.as_str(), r.wpm)).collect();
        assert_eq!(got, [("quietkeys", 130.0), ("sprinter", 120.0)]);
        let mine = me(&pool, Board::First, &scope, a).await.unwrap().unwrap();
        assert_eq!((mine.rank, mine.date.as_deref()), (2, Some("2026-09-27")));
    }
}
