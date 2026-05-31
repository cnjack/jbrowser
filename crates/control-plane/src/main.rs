mod db;
mod server;

use std::net::SocketAddr;
use std::time::Duration;

use anyhow::Context;
use server::{build_router, AppConfig, AppState};
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,jbrowser_control_plane=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    let config = AppConfig::from_env()?;
    let addr: SocketAddr = format!("{}:{}", config.host, config.port)
        .parse()
        .context("invalid bind address")?;

    let pool = sqlx::mysql::MySqlPoolOptions::new()
        .max_connections(20)
        .connect(&config.database_url)
        .await?;

    db::run_migrations(&pool).await?;
    info!("database migrations complete");

    let state = AppState::new(config, pool).await;

    // Background task: expire pending resets after 30 seconds
    {
        let state_clone = state.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            loop {
                interval.tick().await;
                let mut resets = state_clone.pending_resets.write().await;
                resets.retain(|browser_id, started_at| {
                    if started_at.elapsed() > Duration::from_secs(30) {
                        warn!(%browser_id, "reset timed out, clearing pending state");
                        false
                    } else {
                        true
                    }
                });
            }
        });
    }

    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind(addr).await?;

    info!(%addr, "control plane listening");
    axum::serve(listener, app).await?;
    Ok(())
}
