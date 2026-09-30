//! Public profiles (see docs/online-handoff.md §8): personal bests, streak
//! and recent dailies of one user. Private unless the user turned
//! `users.public` on; a user can always see their own.

use anyhow::Result;
use chrono::NaiveDate;
use sqlx::sqlite::{SqlitePool, SqliteRow};
use sqlx::{AssertSqlSafe, Row};
use ttyp_core::api::{Account, Badges, Profile, ProfileRun};

use crate::daily;
use crate::leaderboard::{FIRST_TRY, RANK_ORDER};

/// Latest runs shown on a profile.
const RECENT: i64 = 10;

pub async fn account(pool: &SqlitePool, user_id: i64) -> Result<Account> {
    let row = sqlx::query("SELECT github_login, public FROM users WHERE id = ?1")
        .bind(user_id)
        .fetch_one(pool)
        .await?;
    Ok(Account {
        login: row.get("github_login"),
        public: row.get::<i64, _>("public") != 0,
    })
}

pub async fn set_public(pool: &SqlitePool, user_id: i64, public: bool) -> Result<()> {
    sqlx::query("UPDATE users SET public = ?2 WHERE id = ?1")
        .bind(user_id)
        .bind(i64::from(public))
        .execute(pool)
        .await?;
    Ok(())
}

/// The profile of `login` (GitHub logins ignore case), if it's public or
/// `viewer` is its owner. `None` for unknown and private alike.
pub async fn load(
    pool: &SqlitePool,
    login: &str,
    viewer: Option<i64>,
    today: NaiveDate,
) -> Result<Option<Profile>> {
    let Some(user) = sqlx::query(
        "SELECT id, github_login, public, created_at FROM users \
         WHERE github_login = ?1 COLLATE NOCASE",
    )
    .bind(login)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(None);
    };
    let id: i64 = user.get("id");
    let public = user.get::<i64, _>("public") != 0;
    if !public && viewer != Some(id) {
        return Ok(None);
    }

    let bests = sqlx::query(
        "SELECT r.id, d.date, d.language, d.mode_kind, d.mode_value, r.wpm, r.acc, r.attempt \
         FROM results r JOIN daily_tests d ON d.id = r.daily_id \
         WHERE r.user_id = ?1 AND r.valid = 1 AND r.id = ( \
           SELECT b.id FROM results b JOIN daily_tests bd ON bd.id = b.daily_id \
           WHERE b.user_id = ?1 AND b.valid = 1 AND bd.language = d.language \
             AND bd.mode_kind = d.mode_kind AND bd.mode_value = d.mode_value \
           ORDER BY b.wpm DESC, b.acc DESC, b.created_at ASC LIMIT 1) \
         ORDER BY d.language, d.mode_kind DESC, d.mode_value",
    )
    .bind(id)
    .fetch_all(pool)
    .await?;
    let recent = sqlx::query(
        "SELECT r.id, d.date, d.language, d.mode_kind, d.mode_value, r.wpm, r.acc, r.attempt \
         FROM results r JOIN daily_tests d ON d.id = r.daily_id \
         WHERE r.user_id = ?1 AND r.valid = 1 \
         ORDER BY r.created_at DESC, r.id DESC LIMIT ?2",
    )
    .bind(id)
    .bind(RECENT)
    .fetch_all(pool)
    .await?;
    let dailies: i64 = sqlx::query_scalar(
        "SELECT count(DISTINCT daily_id) FROM results WHERE user_id = ?1 AND valid = 1",
    )
    .bind(id)
    .fetch_one(pool)
    .await?;
    let dates: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT d.date FROM results r JOIN daily_tests d ON d.id = r.daily_id \
         WHERE r.user_id = ?1 AND r.valid = 1 ORDER BY d.date DESC",
    )
    .bind(id)
    .fetch_all(pool)
    .await?;
    let dates: Vec<NaiveDate> = dates.iter().filter_map(|d| d.parse().ok()).collect();

    let badges = badges(pool, id, today).await?;

    let created: String = user.get("created_at");
    Ok(Some(Profile {
        login: user.get("github_login"),
        public,
        joined: created.get(..10).unwrap_or(&created).to_string(),
        streak: streak(&dates, today),
        dailies: dailies as u32,
        bests: bests.iter().filter_map(run).collect(),
        recent: recent.iter().filter_map(run).collect(),
        badges,
    }))
}

/// Top-three places on the first-try board of every daily whose UTC day
/// is over (so standings are final), per language. Ranked exactly like
/// `leaderboard::page`'s first-try board.
async fn badges(pool: &SqlitePool, user_id: i64, today: NaiveDate) -> Result<Vec<Badges>> {
    let sql = format!(
        "WITH ranked AS ( \
           SELECT r.user_id, d.language, \
                  ROW_NUMBER() OVER (PARTITION BY r.daily_id ORDER BY {RANK_ORDER}) AS rank \
           FROM results r JOIN daily_tests d ON d.id = r.daily_id \
           WHERE r.valid = 1 AND {FIRST_TRY} AND d.date < ?2) \
         SELECT language, \
                sum(rank = 1) AS first, sum(rank = 2) AS second, sum(rank = 3) AS third \
         FROM ranked WHERE user_id = ?1 AND rank <= 3 \
         GROUP BY language ORDER BY language"
    );
    let rows = sqlx::query(AssertSqlSafe(sql))
        .bind(user_id)
        .bind(today.to_string())
        .fetch_all(pool)
        .await?;
    Ok(rows
        .iter()
        .map(|r| Badges {
            language: r.get("language"),
            first: r.get::<i64, _>("first") as u32,
            second: r.get::<i64, _>("second") as u32,
            third: r.get::<i64, _>("third") as u32,
        })
        .collect())
}

fn run(row: &SqliteRow) -> Option<ProfileRun> {
    Some(ProfileRun {
        result_id: row.get("id"),
        date: row.get("date"),
        language: row.get("language"),
        mode: daily::mode_from(row.get("mode_kind"), row.get("mode_value"))?,
        wpm: row.get("wpm"),
        acc: row.get("acc"),
        attempt: row.get::<i64, _>("attempt") as u32,
    })
}

/// Consecutive days with a daily, counting back from today, or from
/// yesterday when today has none yet. `dates` is newest first, distinct.
fn streak(dates: &[NaiveDate], today: NaiveDate) -> u32 {
    let Some(&first) = dates.first() else {
        return 0;
    };
    if first != today && Some(first) != today.pred_opt() {
        return 0;
    }
    let mut n = 1;
    for pair in dates.windows(2) {
        if pair[0].pred_opt() == Some(pair[1]) {
            n += 1;
        } else {
            break;
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    async fn user(pool: &SqlitePool, id: i64, login: &str) {
        sqlx::query("INSERT INTO users (id, github_id, github_login, public, created_at) VALUES (?1, ?1, ?2, 1, '2026-09-01')")
            .bind(id)
            .bind(login)
            .execute(pool)
            .await
            .unwrap();
    }

    async fn daily(pool: &SqlitePool, id: i64, date: &str, language: &str) {
        sqlx::query(
            "INSERT INTO daily_tests (id, date, language, mode_kind, mode_value, words, seed, source, created_at) \
             VALUES (?1, ?2, ?3, 'time', 30, '[]', 0, 'test', ?2)",
        )
        .bind(id)
        .bind(date)
        .bind(language)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn result(
        pool: &SqlitePool,
        user: i64,
        daily: i64,
        attempt: i64,
        wpm: f64,
        eligible: bool,
    ) {
        sqlx::query(
            "INSERT INTO results (user_id, daily_id, attempt, wpm, raw, acc, consistency, chars, \
             wpm_per_second, raw_per_second, errors_per_second, keylog, valid, created_at, first_eligible) \
             VALUES (?1, ?2, ?3, ?4, ?4, 100, 90, '{}', '[]', '[]', '[]', x'', 1, '2026-09-29', ?5)",
        )
        .bind(user)
        .bind(daily)
        .bind(attempt)
        .bind(wpm)
        .bind(i64::from(eligible))
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn badges_count_top_three_per_language_on_finished_dailies() {
        let pool = crate::db::open_memory().await;
        for (id, login) in [(1, "ann"), (2, "bo"), (3, "cy"), (4, "di")] {
            user(&pool, id, login).await;
        }
        daily(&pool, 1, "2026-09-29", "english").await;
        daily(&pool, 2, "2026-09-29", "english_1k").await;
        daily(&pool, 3, "2026-09-28", "english").await;
        daily(&pool, 4, "2026-09-30", "english").await; // today: not final yet
        // Yesterday, english: ann 1st, bo 2nd, cy 3rd, di 4th.
        for (u, wpm) in [(1, 100.0), (2, 90.0), (3, 80.0), (4, 70.0)] {
            result(&pool, u, 1, 1, wpm, true).await;
        }
        // A faster second attempt doesn't count: first tries only.
        result(&pool, 4, 1, 2, 200.0, true).await;
        // Yesterday, english_1k: bo 1st, ann 2nd.
        result(&pool, 2, 2, 1, 95.0, true).await;
        result(&pool, 1, 2, 1, 85.0, true).await;
        // Two days ago, english: ann 1st; cy's run never started, so it's
        // not on the board and bo moves up to 2nd.
        result(&pool, 3, 3, 1, 150.0, false).await;
        result(&pool, 1, 3, 1, 99.0, true).await;
        result(&pool, 2, 3, 1, 50.0, true).await;
        // Today: ann would be 1st, but the day isn't over.
        result(&pool, 1, 4, 1, 120.0, true).await;

        let today = "2026-09-30".parse().unwrap();
        let ann = badges(&pool, 1, today).await.unwrap();
        let b = |language: &str, first, second, third| Badges {
            language: language.into(),
            first,
            second,
            third,
        };
        assert_eq!(ann, [b("english", 2, 0, 0), b("english_1k", 0, 1, 0)]);
        assert_eq!(Badges::sum(&ann).total(), 3);
        let bo = badges(&pool, 2, today).await.unwrap();
        assert_eq!(bo, [b("english", 0, 2, 0), b("english_1k", 1, 0, 0)]);
        assert_eq!(
            badges(&pool, 3, today).await.unwrap(),
            [b("english", 0, 0, 1)]
        );
        assert!(
            badges(&pool, 4, today).await.unwrap().is_empty(),
            "4th earns nothing"
        );
        let profile = load(&pool, "ann", None, today).await.unwrap().unwrap();
        assert_eq!(profile.badges, ann);
    }

    #[test]
    fn streak_counts_back_from_today_or_yesterday() {
        let today = d("2026-09-30");
        assert_eq!(streak(&[], today), 0);
        let run = [
            d("2026-09-30"),
            d("2026-09-29"),
            d("2026-09-28"),
            d("2026-09-25"),
        ];
        assert_eq!(streak(&run, today), 3);
        // Not played yet today: yesterday's streak still stands.
        assert_eq!(streak(&run[1..], today), 2);
        // Last played two days ago: broken.
        assert_eq!(streak(&run[2..], today), 0);
    }
}
