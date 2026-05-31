mod cdp_tunnel;
mod chrome;
mod commands;
mod config;
mod connection;
mod globals;
mod input;
mod screencast;

use std::collections::HashMap;
use std::sync::Arc;

use bytes::Bytes;
use jbrowser_shared::models::BrowserConfig;
use tokio::sync::{broadcast, watch, Mutex, RwLock};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use config::{load_or_register, save_identity, AgentConfig};
use globals::{ACTIVE_TAB_TX, BROWSER_CONFIG, CDP_TUNNELS, INPUT_TX};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,jbrowser_agent=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = AgentConfig::from_env()?;
    tokio::fs::create_dir_all(&config.data_dir).await?;
    let identity = load_or_register(&config).await?;
    save_identity(&config, &identity).await?;

    // Initialize browser config (default; will be updated from control plane in future)
    let browser_cfg = BrowserConfig::default();
    BROWSER_CONFIG
        .set(Arc::new(RwLock::new(browser_cfg.clone())))
        .expect("BROWSER_CONFIG already set");

    let _chrome = chrome::start_chrome(&browser_cfg).await?;

    let (video_tx, _) = broadcast::channel::<Bytes>(16);
    let last_frame: Arc<Mutex<Option<Bytes>>> = Arc::new(Mutex::new(None));

    let (active_tab_tx, _) = watch::channel::<Option<String>>(None);
    ACTIVE_TAB_TX
        .set(active_tab_tx)
        .expect("ACTIVE_TAB_TX already set");

    let (input_chan_tx, input_chan_rx) = tokio::sync::mpsc::channel::<serde_json::Value>(256);
    INPUT_TX.set(input_chan_tx).expect("input_tx already set");
    tokio::spawn(input::input_loop(input_chan_rx));

    CDP_TUNNELS
        .set(Arc::new(RwLock::new(HashMap::new())))
        .expect("CDP_TUNNELS already set");

    tokio::spawn(screencast::screencast_loop(
        video_tx.clone(),
        last_frame.clone(),
    ));

    connection::connect_loop(config, identity, video_tx, last_frame).await
}
