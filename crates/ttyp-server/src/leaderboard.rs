//! The two boards of a daily. *First try* ranks each user's attempt 1
//! (when the server saw it start, see `api::start`);
//! *best* ranks each user's highest-wpm valid run. Both order by wpm, then
//! accuracy, then who got there first.

use anyhow::Result;
use sqlx::sqlite::{SqlitePool, SqliteRow};
use sqlx::{AssertSqlSafe, Row};
use ttyp_core::api::{Board, LeaderboardRow};

/// Which `results r` rows are on the first-try board (plus `r.valid = 1`).
pub const FIRST_TRY: &str = "r.attempt = 1 AND r.first_eligible = 1";
/// How every board orders its rows.
pub const RANK_ORDER: &str = "r.wpm DESC, r.acc DESC, r.created_at ASC";

/// The ranked rows of one board as a common table expression `board`.
fn board_cte(board: Board) -> String {
    let filter = match board {
        Board::First => FIRST_TRY,
        // Each user's best run: the one that sorts first among theirs.
        Board::Best => {
            "r.id = (SELECT b.id FROM results b \
             WHERE b.user_id = r.user_id AND b.daily_id = r.daily_id AND b.valid = 1 \
             ORDER BY b.wpm DESC, b.acc DESC, b.created_at ASC LIMIT 1)"
        }
    };
    format!(
        "WITH board AS (\
           SELECT r.id AS result_id, r.user_id, u.github_login AS user, \
                  r.wpm, r.raw, r.acc, r.consistency, \
                  ROW_NUMBER() OVER (ORDER BY {RANK_ORDER}) AS rank \
           FROM results r JOIN users u ON u.id = r.user_id \
           WHERE r.daily_id = ?1 AND r.valid = 1 AND {filter}) "
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
    }
}

/// `limit` rows from `offset`, plus the board's size.
pub async fn page(
    pool: &SqlitePool,
    daily_id: i64,
    board: Board,
    offset: u32,
    limit: u32,
) -> Result<(Vec<LeaderboardRow>, u32)> {
    let sql = board_cte(board)
        + "SELECT *, (SELECT count(*) FROM board) AS total FROM board \
           ORDER BY rank LIMIT ?2 OFFSET ?3";
    let rows = sqlx::query(AssertSqlSafe(sql))
        .bind(daily_id)
        .bind(i64::from(limit))
        .bind(i64::from(offset))
        .fetch_all(pool)
        .await?;
    let total = match rows.first() {
        Some(r) => r.get::<i64, _>("total") as u32,
        // Past the end: count separately.
        None => {
            let sql = board_cte(board) + "SELECT count(*) FROM board";
            sqlx::query_scalar::<_, i64>(AssertSqlSafe(sql))
                .bind(daily_id)
                .fetch_one(pool)
                .await? as u32
        }
    };
    Ok((rows.iter().map(row_from).collect(), total))
}

/// One user's row on a board.
pub async fn me(
    pool: &SqlitePool,
    daily_id: i64,
    board: Board,
    user_id: i64,
) -> Result<Option<LeaderboardRow>> {
    let sql = board_cte(board) + "SELECT * FROM board WHERE user_id = ?2";
    let row = sqlx::query(AssertSqlSafe(sql))
        .bind(daily_id)
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
        let langs = ttyp_core::language::LanguageRegistry::builtin();
        let date = chrono::NaiveDate::from_ymd_opt(2026, 9, 29).unwrap();
        crate::daily::insert(
            &pool.clone(),
            &langs,
            date,
            "english",
            ttyp_core::test::Mode::Words(10),
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

        let (first, total) = page(&pool, d, Board::First, 0, 50).await.unwrap();
        assert_eq!(total, 4);
        let names: Vec<&str> = first.iter().map(|r| r.user.as_str()).collect();
        assert_eq!(names, ["tie", "sprinter", "quietkeys", "andples"]);
        assert_eq!(first[1].rank, 2);

        let (best, total) = page(&pool, d, Board::Best, 0, 50).await.unwrap();
        assert_eq!(total, 4);
        let names: Vec<&str> = best.iter().map(|r| r.user.as_str()).collect();
        assert_eq!(names, ["quietkeys", "tie", "sprinter", "andples"]);
        assert_eq!(best[0].wpm, 151.0);
        assert_eq!(best[3].wpm, 131.0, "best, not latest");

        let (page2, total) = page(&pool, d, Board::Best, 3, 2).await.unwrap();
        assert_eq!((page2.len(), total), (1, 4));
        assert_eq!(page2[0].rank, 4);
        let (empty, total) = page(&pool, d, Board::Best, 10, 2).await.unwrap();
        assert_eq!((empty.len(), total), (0, 4));

        let mine = me(&pool, d, Board::Best, c).await.unwrap().unwrap();
        assert_eq!((mine.rank, mine.wpm), (4, 131.0));
        assert_eq!(me(&pool, d, Board::First, 999).await.unwrap(), None);
    }
}
