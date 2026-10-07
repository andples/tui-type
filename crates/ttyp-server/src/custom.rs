//! Published custom word sets (`GET /custom`, `GET /custom/{name}`,
//! `POST /custom`, `POST /custom/{name}/install`, `.../unpublish`). Sets are
//! ranked by installs: each logged-in player counts once, the author never.

use anyhow::Result;
use sqlx::Row;
use sqlx::sqlite::{SqlitePool, SqliteRow};
use ttyp_core::api::{CustomList, CustomSet, CustomSummary};

use crate::db;

/// Most sets one account may publish.
pub const MAX_SETS: i64 = 50;

/// Why a publish or unpublish was refused.
#[derive(Debug, PartialEq, Eq)]
pub enum CustomError {
    Invalid(String),
    /// Someone else owns that name.
    Taken,
    TooMany,
    NotFound,
    NotYours,
}

const SUMMARY: &str = "SELECT s.id, s.name, u.github_login AS author, s.word_count, s.updated_at, \
       (SELECT count(*) FROM custom_installs i WHERE i.set_id = s.id) AS installs \
     FROM custom_sets s JOIN users u ON u.id = s.owner_id";

fn summary(row: &SqliteRow) -> CustomSummary {
    let updated: String = row.get("updated_at");
    CustomSummary {
        name: row.get("name"),
        author: row.get("author"),
        words: row.get::<i64, _>("word_count") as u32,
        installs: row.get::<i64, _>("installs") as u32,
        updated: updated.get(..10).unwrap_or(&updated).to_string(),
    }
}

/// Sets whose name or author contains `query`, most installed first.
pub async fn list(pool: &SqlitePool, query: &str, offset: u32, limit: u32) -> Result<CustomList> {
    let like = format!("%{}%", escape_like(query.trim()));
    let filter = "WHERE s.name LIKE ?1 ESCAPE '\\' OR u.github_login LIKE ?1 ESCAPE '\\'";
    let total: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT count(*) FROM custom_sets s JOIN users u ON u.id = s.owner_id {filter}"
    )))
    .bind(&like)
    .fetch_one(pool)
    .await?;
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "{SUMMARY} {filter} ORDER BY installs DESC, s.updated_at DESC, s.name LIMIT ?2 OFFSET ?3"
    )))
    .bind(&like)
    .bind(i64::from(limit))
    .bind(i64::from(offset))
    .fetch_all(pool)
    .await?;
    Ok(CustomList {
        rows: rows.iter().map(summary).collect(),
        offset,
        total: total as u32,
    })
}

/// A set with its words; with `installer`, count them as installing it.
pub async fn get(
    pool: &SqlitePool,
    name: &str,
    installer: Option<i64>,
) -> Result<Option<CustomSet>> {
    let Some(row) = sqlx::query(
        "SELECT s.id, s.name, s.owner_id, s.words, u.github_login AS author \
         FROM custom_sets s JOIN users u ON u.id = s.owner_id WHERE s.name = ?1",
    )
    .bind(name)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(None);
    };
    let id: i64 = row.get("id");
    if let Some(user) = installer.filter(|u| *u != row.get::<i64, _>("owner_id")) {
        sqlx::query(
            "INSERT INTO custom_installs (set_id, user_id, created_at) VALUES (?1, ?2, ?3) \
             ON CONFLICT DO NOTHING",
        )
        .bind(id)
        .bind(user)
        .bind(db::now())
        .execute(pool)
        .await?;
    }
    let installs: i64 =
        sqlx::query_scalar("SELECT count(*) FROM custom_installs WHERE set_id = ?1")
            .bind(id)
            .fetch_one(pool)
            .await?;
    let words: String = row.get("words");
    Ok(Some(CustomSet {
        name: row.get("name"),
        author: row.get("author"),
        words: serde_json::from_str(&words).unwrap_or_default(),
        installs: installs as u32,
    }))
}

/// Publish `words` as `name`, or a new version of the owner's own set.
pub async fn publish(
    pool: &SqlitePool,
    owner: i64,
    name: &str,
    words: &[String],
) -> Result<Result<CustomSummary, CustomError>> {
    if let Err(e) = ttyp_core::custom::check(name, words) {
        return Ok(Err(CustomError::Invalid(e)));
    }
    let existing = sqlx::query("SELECT id, owner_id FROM custom_sets WHERE name = ?1")
        .bind(name)
        .fetch_optional(pool)
        .await?;
    let json = serde_json::to_string(words)?;
    let now = db::now();
    match existing {
        Some(row) if row.get::<i64, _>("owner_id") != owner => return Ok(Err(CustomError::Taken)),
        Some(row) => {
            sqlx::query(
                "UPDATE custom_sets SET words = ?2, word_count = ?3, updated_at = ?4 WHERE id = ?1",
            )
            .bind(row.get::<i64, _>("id"))
            .bind(&json)
            .bind(words.len() as i64)
            .bind(&now)
            .execute(pool)
            .await?;
        }
        None => {
            let mine: i64 =
                sqlx::query_scalar("SELECT count(*) FROM custom_sets WHERE owner_id = ?1")
                    .bind(owner)
                    .fetch_one(pool)
                    .await?;
            if mine >= MAX_SETS {
                return Ok(Err(CustomError::TooMany));
            }
            sqlx::query(
                "INSERT INTO custom_sets (name, owner_id, words, word_count, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            )
            .bind(name)
            .bind(owner)
            .bind(&json)
            .bind(words.len() as i64)
            .bind(&now)
            .execute(pool)
            .await?;
        }
    }
    let row = sqlx::query(sqlx::AssertSqlSafe(format!("{SUMMARY} WHERE s.name = ?1")))
        .bind(name)
        .fetch_one(pool)
        .await?;
    Ok(Ok(summary(&row)))
}

/// Take the owner's set down (its install count goes with it).
pub async fn unpublish(
    pool: &SqlitePool,
    owner: i64,
    name: &str,
) -> Result<Result<(), CustomError>> {
    let Some(row) = sqlx::query("SELECT id, owner_id FROM custom_sets WHERE name = ?1")
        .bind(name)
        .fetch_optional(pool)
        .await?
    else {
        return Ok(Err(CustomError::NotFound));
    };
    if row.get::<i64, _>("owner_id") != owner {
        return Ok(Err(CustomError::NotYours));
    }
    sqlx::query("DELETE FROM custom_sets WHERE id = ?1")
        .bind(row.get::<i64, _>("id"))
        .execute(pool)
        .await?;
    Ok(Ok(()))
}

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
