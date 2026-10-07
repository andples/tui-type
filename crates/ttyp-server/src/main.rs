//! ttyp-server: generates the daily tests, scores submitted keylogs by
//! replaying them and serves the leaderboards. Configured by environment:
//! `TTYP_BIND` (default `127.0.0.1:8080`), `TTYP_DB` (default `ttyp.db`),
//! `RUST_LOG`. `ttyp-server healthcheck` probes a running server (for the
//! container healthcheck; the image has no curl).

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;
use ttyp_core::language::LanguageRegistry;

mod api;
mod auth;
mod custom;
mod daily;
mod db;
mod leaderboard;
mod players;
mod profile;

#[tokio::main]
async fn main() -> Result<()> {
    if std::env::args().nth(1).as_deref() == Some("healthcheck") {
        return healthcheck();
    }
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let bind = std::env::var("TTYP_BIND").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let db_path = PathBuf::from(std::env::var("TTYP_DB").unwrap_or_else(|_| "ttyp.db".into()));

    let pool = db::open(&db_path)
        .await
        .with_context(|| format!("opening {}", db_path.display()))?;
    let languages = Arc::new(LanguageRegistry::builtin());
    let state = api::AppState::new(pool.clone(), languages.clone());

    // Heal a missed midnight before serving, then keep the clock.
    daily::ensure_today(&pool, &languages).await?;
    tokio::spawn(daily::scheduler(pool, languages));

    let listener = TcpListener::bind(&bind)
        .await
        .with_context(|| format!("binding {bind}"))?;
    tracing::info!("listening on {bind}, db {}", db_path.display());
    axum::serve(listener, api::router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

/// `GET /health` on this host's `TTYP_BIND` port; exits non-zero on failure.
fn healthcheck() -> Result<()> {
    let bind = std::env::var("TTYP_BIND").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let port = bind.rsplit(':').next().unwrap_or("8080");
    let mut res = ureq::get(&format!("http://127.0.0.1:{port}/health"))
        .call()
        .context("health request")?;
    let body = res.body_mut().read_to_string()?;
    anyhow::ensure!(body.contains("\"ok\""), "unhealthy: {body}");
    println!("{body}");
    Ok(())
}

/// Ctrl-C or SIGTERM (what `docker stop` sends).
async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut term = signal(SignalKind::terminate()).expect("install SIGTERM handler");
        tokio::select! {
            _ = ctrl_c => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    let _ = ctrl_c.await;
    tracing::info!("shutting down");
}
