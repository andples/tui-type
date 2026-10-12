//! Account sync (`GET`/`POST /sync/configs`, `GET`/`POST /sync/bests`):
//! each player's configs and personal bests, private to them. Configs are
//! one JSON blob with a revision, replaced only by a push that saw the
//! latest revision; bests only ever get faster.

use anyhow::Result;
use sqlx::Row;
use sqlx::sqlite::SqlitePool;
use ttyp_core::sync::{Best, ConfigSync, Configs};

use crate::daily::mode_from;
use crate::db;

/// The player's configs, or an empty revision 0 before their first push.
pub async fn configs(pool: &SqlitePool, user_id: i64) -> Result<ConfigSync> {
    let row = sqlx::query("SELECT rev, configs FROM config_sync WHERE user_id = ?1")
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    Ok(match row {
        Some(row) => ConfigSync {
            rev: row.get("rev"),
            configs: serde_json::from_str(row.get::<&str, _>("configs"))?,
        },
        None => ConfigSync::default(),
    })
}

/// Store `configs` if the player's are still at `base_rev`. Answers with
/// what the server has afterwards, either way.
pub async fn push_configs(
    pool: &SqlitePool,
    user_id: i64,
    base_rev: i64,
    configs: &Configs,
) -> Result<ConfigSync> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let rev: i64 = sqlx::query_scalar("SELECT rev FROM config_sync WHERE user_id = ?1")
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or(0);
    if rev == base_rev {
        sqlx::query(
            "INSERT INTO config_sync (user_id, rev, configs, updated_at) VALUES (?1, ?2, ?3, ?4) \
             ON CONFLICT (user_id) DO UPDATE SET \
               rev = excluded.rev, configs = excluded.configs, updated_at = excluded.updated_at",
        )
        .bind(user_id)
        .bind(rev + 1)
        .bind(serde_json::to_string(configs)?)
        .bind(db::now())
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    self::configs(pool, user_id).await
}

/// Every best the player has, by mode then language.
pub async fn bests(pool: &SqlitePool, user_id: i64) -> Result<Vec<Best>> {
    let rows = sqlx::query(
        "SELECT mode_kind, mode_value, language, wpm FROM bests WHERE user_id = ?1 \
         ORDER BY mode_kind, mode_value, language",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .filter_map(|row| {
            Some(Best {
                mode: mode_from(row.get("mode_kind"), row.get("mode_value"))?,
                language: row.get("language"),
                wpm: row.get("wpm"),
            })
        })
        .collect())
}

/// Keep each of `new` that beats (or is the first for) its mode and
/// language, as long as the player stays under `max` bests.
pub async fn push_bests(pool: &SqlitePool, user_id: i64, new: &[Best], max: usize) -> Result<()> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let mut count: i64 = sqlx::query_scalar("SELECT count(*) FROM bests WHERE user_id = ?1")
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await?;
    let now = db::now();
    for b in new {
        let (kind, value) = (b.mode.kind(), i64::from(b.mode.value()));
        let old: Option<f64> = sqlx::query_scalar(
            "SELECT wpm FROM bests \
             WHERE user_id = ?1 AND mode_kind = ?2 AND mode_value = ?3 AND language = ?4",
        )
        .bind(user_id)
        .bind(kind)
        .bind(value)
        .bind(&b.language)
        .fetch_optional(&mut *tx)
        .await?;
        match old {
            Some(w) if w >= b.wpm => continue,
            Some(_) => {}
            None if count as usize >= max => continue,
            None => count += 1,
        }
        sqlx::query(
            "INSERT INTO bests (user_id, mode_kind, mode_value, language, wpm, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
             ON CONFLICT (user_id, mode_kind, mode_value, language) DO UPDATE SET \
               wpm = excluded.wpm, updated_at = excluded.updated_at",
        )
        .bind(user_id)
        .bind(kind)
        .bind(value)
        .bind(&b.language)
        .bind(b.wpm)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}
