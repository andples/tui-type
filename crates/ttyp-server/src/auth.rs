//! Bearer tokens: a random 32-byte token whose sha256 is stored in
//! `api_tokens`. The GitHub exchange that issues them lives in `api`.

use anyhow::Result;
use axum::http::HeaderMap;
use rand::RngExt;
use sha2::{Digest, Sha256};
use sqlx::Row;
use sqlx::sqlite::SqlitePool;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    pub id: i64,
    pub login: String,
}

/// Base64url of 32 random bytes (43 chars, no padding).
// Issuing and revoking are wired to `/auth/github` and `/auth/logout` in the
// next phase; only verification is routed so far.
#[allow(dead_code)]
pub fn new_token() -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut rng = rand::rng();
    (0..43)
        .map(|_| ALPHABET[rng.random_range(0..ALPHABET.len())] as char)
        .collect()
}

pub fn hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

/// Create or refresh the user for a GitHub account and issue a token.
#[allow(dead_code)]
pub async fn issue(pool: &SqlitePool, github_id: i64, login: &str) -> Result<(User, String)> {
    let now = crate::db::now();
    sqlx::query(
        "INSERT INTO users (github_id, github_login, created_at) VALUES (?1, ?2, ?3) \
         ON CONFLICT (github_id) DO UPDATE SET github_login = excluded.github_login",
    )
    .bind(github_id)
    .bind(login)
    .bind(&now)
    .execute(pool)
    .await?;
    let id: i64 = sqlx::query_scalar("SELECT id FROM users WHERE github_id = ?1")
        .bind(github_id)
        .fetch_one(pool)
        .await?;
    let token = new_token();
    sqlx::query("INSERT INTO api_tokens (token_hash, user_id, created_at) VALUES (?1, ?2, ?3)")
        .bind(hash(&token))
        .bind(id)
        .bind(&now)
        .execute(pool)
        .await?;
    Ok((
        User {
            id,
            login: login.to_string(),
        },
        token,
    ))
}

/// The user behind `Authorization: Bearer …`, if the header is present and
/// the token is known. A malformed or unknown token is `Ok(None)`.
pub async fn user_from_headers(pool: &SqlitePool, headers: &HeaderMap) -> Result<Option<User>> {
    let Some(token) = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|t| !t.is_empty())
    else {
        return Ok(None);
    };
    let hash = hash(token);
    let row = sqlx::query(
        "SELECT u.id, u.github_login FROM api_tokens t JOIN users u ON u.id = t.user_id \
         WHERE t.token_hash = ?1",
    )
    .bind(&hash)
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else { return Ok(None) };
    sqlx::query("UPDATE api_tokens SET last_used = ?1 WHERE token_hash = ?2")
        .bind(crate::db::now())
        .bind(&hash)
        .execute(pool)
        .await?;
    Ok(Some(User {
        id: row.get("id"),
        login: row.get("github_login"),
    }))
}

#[allow(dead_code)]
pub async fn revoke(pool: &SqlitePool, token: &str) -> Result<bool> {
    let done = sqlx::query("DELETE FROM api_tokens WHERE token_hash = ?1")
        .bind(hash(token))
        .execute(pool)
        .await?;
    Ok(done.rows_affected() > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[tokio::test]
    async fn tokens_round_trip() {
        let pool = crate::db::open_memory().await;
        let (user, token) = issue(&pool, 42, "octocat").await.unwrap();
        assert_eq!(token.len(), 43);
        let mut h = HeaderMap::new();
        assert_eq!(user_from_headers(&pool, &h).await.unwrap(), None);
        h.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
        );
        assert_eq!(
            user_from_headers(&pool, &h).await.unwrap(),
            Some(user.clone())
        );
        // Logging in again refreshes the login and keeps the id.
        let (again, _) = issue(&pool, 42, "octocat2").await.unwrap();
        assert_eq!(again.id, user.id);
        assert_eq!(again.login, "octocat2");
        assert!(revoke(&pool, &token).await.unwrap());
        assert_eq!(user_from_headers(&pool, &h).await.unwrap(), None);
    }
}
