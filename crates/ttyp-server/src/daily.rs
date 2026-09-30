//! Daily test generation: one row per enabled schedule entry per UTC day,
//! created at midnight or backfilled whenever they're found missing.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::{NaiveDate, Utc};
use rand::RngExt;
use sqlx::Row;
use sqlx::sqlite::SqlitePool;
use ttyp_core::api::{Daily, DailySummary};
use ttyp_core::language::{Language, LanguageRegistry};
use ttyp_core::test::Mode;
use ttyp_core::test::{Modifiers, RandomGenerator, WordGenerator};

/// Words stored for a time-mode daily: enough for 350 wpm.
pub fn words_for(mode: Mode) -> usize {
    match mode {
        Mode::Words(n) => n as usize,
        Mode::Time(secs) => (secs as usize * 350).div_ceil(60),
    }
}

pub fn today() -> NaiveDate {
    Utc::now().date_naive()
}

/// `(mode_kind, mode_value)` columns back to a `Mode`.
pub fn mode_from(kind: &str, value: i64) -> Option<Mode> {
    let value = u16::try_from(value).ok()?;
    match kind {
        "time" => Some(Mode::Time(value)),
        "words" => Some(Mode::Words(value)),
        _ => None,
    }
}

/// The plain (no punctuation, no numbers) words of a daily.
pub fn generate_words(language: &Language, mode: Mode, seed: u64) -> Vec<String> {
    let mut g = RandomGenerator::with_seed(language.words.clone(), Modifiers::default(), seed);
    g.next_words(words_for(mode))
}

/// Insert the daily for `(date, language, mode)` unless it exists. Returns
/// its id when a row was created. Usable for any pair, not just the
/// schedule, so on-request dailies can share it later.
pub async fn insert(
    pool: &SqlitePool,
    languages: &LanguageRegistry,
    date: NaiveDate,
    language: &str,
    mode: Mode,
    source: &str,
    requested_by: Option<i64>,
) -> Result<Option<i64>> {
    let lang = languages
        .get(language)
        .with_context(|| format!("unknown language `{language}`"))?;
    let seed: u64 = rand::rng().random();
    let words = serde_json::to_string(&generate_words(lang, mode, seed))?;
    let done = sqlx::query(
        "INSERT INTO daily_tests \
         (date, language, mode_kind, mode_value, words, seed, source, requested_by, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
         ON CONFLICT (date, language, mode_kind, mode_value) DO NOTHING",
    )
    .bind(date.to_string())
    .bind(language)
    .bind(mode.kind())
    .bind(i64::from(mode.value()))
    .bind(words)
    // SQLite integers are signed; keep the bit pattern.
    .bind(seed as i64)
    .bind(source)
    .bind(requested_by)
    .bind(crate::db::now())
    .execute(pool)
    .await?;
    Ok((done.rows_affected() > 0).then(|| done.last_insert_rowid()))
}

/// Create every enabled scheduled daily for `date` that doesn't exist yet.
/// Returns how many were created. Safe to call any time.
pub async fn ensure(
    pool: &SqlitePool,
    languages: &LanguageRegistry,
    date: NaiveDate,
) -> Result<usize> {
    let wanted = sqlx::query(
        "SELECT s.language, s.mode_kind, s.mode_value FROM daily_schedule s \
         WHERE s.enabled = 1 AND NOT EXISTS (\
           SELECT 1 FROM daily_tests d WHERE d.date = ?1 AND d.language = s.language \
           AND d.mode_kind = s.mode_kind AND d.mode_value = s.mode_value)",
    )
    .bind(date.to_string())
    .fetch_all(pool)
    .await?;
    let mut created = 0;
    for row in wanted {
        let language: String = row.get("language");
        let kind: String = row.get("mode_kind");
        let value: i64 = row.get("mode_value");
        let Some(mode) = mode_from(&kind, value) else {
            tracing::warn!("schedule row with unknown mode {kind} {value}");
            continue;
        };
        if insert(pool, languages, date, &language, mode, "schedule", None)
            .await?
            .is_some()
        {
            created += 1;
        }
    }
    if created > 0 {
        tracing::info!("created {created} dailies for {date}");
    }
    Ok(created)
}

pub async fn ensure_today(pool: &SqlitePool, languages: &LanguageRegistry) -> Result<usize> {
    ensure(pool, languages, today()).await
}

/// Sleep until the next 00:00 UTC, generate, repeat.
pub async fn scheduler(pool: SqlitePool, languages: Arc<LanguageRegistry>) {
    loop {
        let now = Utc::now();
        let next = (now.date_naive() + chrono::Days::new(1))
            .and_hms_opt(0, 0, 0)
            .expect("midnight")
            .and_utc();
        let wait = (next - now).to_std().unwrap_or(Duration::ZERO);
        tokio::time::sleep(wait).await;
        if let Err(e) = ensure_today(&pool, &languages).await {
            tracing::error!("daily generation failed: {e:#}");
        }
    }
}

/// Today's dailies (backfilling first), oldest schedule order.
pub async fn list(pool: &SqlitePool, date: NaiveDate) -> Result<Vec<DailySummary>> {
    let rows = sqlx::query(
        "SELECT id, date, language, mode_kind, mode_value FROM daily_tests \
         WHERE date = ?1 ORDER BY language, mode_kind DESC, mode_value",
    )
    .bind(date.to_string())
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().filter_map(summary_from).collect())
}

fn summary_from(row: &sqlx::sqlite::SqliteRow) -> Option<DailySummary> {
    Some(DailySummary {
        id: row.get("id"),
        date: row.get("date"),
        language: row.get("language"),
        mode: mode_from(&row.get::<String, _>("mode_kind"), row.get("mode_value"))?,
    })
}

pub async fn get(pool: &SqlitePool, id: i64) -> Result<Option<Daily>> {
    let row = sqlx::query(
        "SELECT id, date, language, mode_kind, mode_value, words FROM daily_tests WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else { return Ok(None) };
    let Some(summary) = summary_from(&row) else {
        return Ok(None);
    };
    let words: Vec<String> = serde_json::from_str(&row.get::<String, _>("words"))?;
    Ok(Some(Daily {
        id: summary.id,
        date: summary.date,
        language: summary.language,
        mode: summary.mode,
        words,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_mode_stores_enough_words() {
        assert_eq!(words_for(Mode::Time(30)), 175);
        assert_eq!(words_for(Mode::Time(15)), 88);
        assert_eq!(words_for(Mode::Words(25)), 25);
    }

    #[tokio::test]
    async fn ensure_creates_14_once() {
        let pool = crate::db::open_memory().await;
        let langs = LanguageRegistry::builtin();
        let date = NaiveDate::from_ymd_opt(2026, 9, 29).unwrap();
        assert_eq!(ensure(&pool, &langs, date).await.unwrap(), 14);
        assert_eq!(ensure(&pool, &langs, date).await.unwrap(), 0, "idempotent");
        let list = list(&pool, date).await.unwrap();
        assert_eq!(list.len(), 14);
        let d = get(&pool, list[0].id).await.unwrap().unwrap();
        assert_eq!(d.words.len(), words_for(d.mode));
        assert!(
            d.words
                .iter()
                .all(|w| w.chars().all(|c| c.is_ascii_lowercase()))
        );
        // A missing one is backfilled on the next call.
        sqlx::query("DELETE FROM daily_tests WHERE id = ?1")
            .bind(list[0].id)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(ensure(&pool, &langs, date).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn every_daily_has_its_own_words() {
        let pool = crate::db::open_memory().await;
        let langs = LanguageRegistry::builtin();
        let date = NaiveDate::from_ymd_opt(2026, 9, 29).unwrap();
        ensure(&pool, &langs, date).await.unwrap();
        let mut dailies = Vec::new();
        for d in list(&pool, date).await.unwrap() {
            dailies.push(get(&pool, d.id).await.unwrap().unwrap());
        }
        // Not the same seed stretched to each length: no daily opens with
        // another's first ten words.
        for (i, a) in dailies.iter().enumerate() {
            for b in &dailies[i + 1..] {
                assert_ne!(
                    a.words[..10],
                    b.words[..10],
                    "{} {} and {} {} share a start",
                    a.language,
                    a.mode.label(),
                    b.language,
                    b.mode.label()
                );
            }
        }
    }
}
