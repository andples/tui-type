//! Public profiles (see docs/online-handoff.md §8): personal bests, streak
//! and recent dailies of one user. Private unless the user turned
//! `users.public` on; a user can always see their own.

use anyhow::Result;
use chrono::NaiveDate;
use sqlx::Row;
use sqlx::sqlite::{SqlitePool, SqliteRow};
use ttyp_core::api::{Account, Profile, ProfileRun};

use crate::daily;

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

    let created: String = user.get("created_at");
    Ok(Some(Profile {
        login: user.get("github_login"),
        public,
        joined: created.get(..10).unwrap_or(&created).to_string(),
        streak: streak(&dates, today),
        dailies: dailies as u32,
        bests: bests.iter().filter_map(run).collect(),
        recent: recent.iter().filter_map(run).collect(),
    }))
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
