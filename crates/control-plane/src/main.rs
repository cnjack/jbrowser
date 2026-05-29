mod server;

use std::net::SocketAddr;

use anyhow::Context;
use server::{build_router, AppConfig, AppState};
use tracing::info;
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
    let state = AppState::new(config);
    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind(addr).await?;

    info!(%addr, "control plane listening");
    axum::serve(listener, app).await?;
    Ok(())
}
