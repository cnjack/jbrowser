use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use jbrowser_shared::protocol::ControlToAgentMessage;
use serde_json::json;
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio::time::interval;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{info, warn};

use crate::chrome::{cdp_get_targets, cdp_to_browser_tabs};
use crate::commands::dispatch_control_message;
use crate::config::{load_or_register, save_identity, AgentConfig, AgentIdentity};
use crate::globals::CONTROL_TX;

pub(crate) async fn connect_loop(
    config: AgentConfig,
    mut identity: AgentIdentity,
    video_tx: broadcast::Sender<Bytes>,
    last_frame: Arc<Mutex<Option<Bytes>>>,
) -> anyhow::Result<()> {
    loop {
        match connect_once(&config, &identity, video_tx.clone(), last_frame.clone()).await {
            Ok(()) => warn!("agent websocket disconnected"),
            Err(err) => {
                let msg = err.to_string();
                warn!(error = %msg, "agent websocket failed");
                if msg.contains("401") || msg.contains("Unauthorized") {
                    warn!("runtime token rejected — deleting identity and re-registering");
                    let id_path = config.data_dir.join("identity.json");
                    let _ = tokio::fs::remove_file(&id_path).await;
                    match load_or_register(&config).await {
                        Ok(new_id) => {
                            if let Err(e) = save_identity(&config, &new_id).await {
                                warn!("failed to save new identity: {e}");
                            }
                            identity = new_id;
                        }
                        Err(e) => warn!("re-registration failed: {e}"),
                    }
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

async fn connect_once(
    config: &AgentConfig,
    identity: &AgentIdentity,
    video_tx: broadcast::Sender<Bytes>,
    _last_frame: Arc<Mutex<Option<Bytes>>>,
) -> anyhow::Result<()> {
    let request = http_request_with_bearer(
        &format!("{}/api/v1/agents/connect", config.control_ws_url),
        &identity.agent_runtime_token,
    )?;
    let (socket, _) = connect_async(request).await?;
    let (mut ws_write, mut ws_read) = socket.split();
    info!("agent websocket connected");

    let (tx, mut rx) = mpsc::channel::<Message>(256);

    {
        let lock = CONTROL_TX.get_or_init(|| Mutex::new(None));
        *lock.lock().await = Some(tx.clone());
    }

    let writer_handle = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_write.send(msg).await.is_err() {
                break;
            }
        }
    });

    let video_rx = video_tx.subscribe();
    let video_handle = tokio::spawn(video_stream_task(video_rx, tx.clone()));

    let tab_handle = tokio::spawn(tab_poll_task(tx.clone(), identity.clone()));

    let mut heartbeat = interval(Duration::from_secs(5));

    let result: anyhow::Result<()> = loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                let msg = json!({
                    "type": "heartbeat",
                    "payload": {
                        "agent_id": identity.agent_id,
                        "browser_instance_id": identity.browser_instance_id
                    }
                });
                if tx.send(Message::Text(msg.to_string())).await.is_err() {
                    break Ok(());
                }
            }
            maybe_msg = ws_read.next() => {
                match maybe_msg {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<ControlToAgentMessage>(&text) {
                            Ok(msg) => { tokio::spawn(dispatch_control_message(msg)); }
                            Err(e) => warn!("failed to parse control message: {e}"),
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break Ok(()),
                    Some(Err(e)) => break Err(anyhow::Error::from(e)),
                    _ => {}
                }
            }
        }
    };

    writer_handle.abort();
    video_handle.abort();
    tab_handle.abort();
    result
}

async fn tab_poll_task(tx: mpsc::Sender<Message>, _identity: AgentIdentity) {
    let mut ticker = interval(Duration::from_secs(2));
    let mut prev_fingerprints: Vec<String> = Vec::new();

    loop {
        ticker.tick().await;

        let targets = match cdp_get_targets().await {
            Ok(t) => t,
            Err(e) => {
                warn!("tab poll failed: {e}");
                continue;
            }
        };

        let tabs = cdp_to_browser_tabs(&targets);
        let fingerprints: Vec<String> = tabs
            .iter()
            .map(|t| format!("{}|{}|{}", t.id, t.url, t.title))
            .collect();

        if fingerprints != prev_fingerprints {
            prev_fingerprints = fingerprints;
            let msg = json!({ "type": "tab.list", "payload": { "tabs": tabs } });
            if tx.send(Message::Text(msg.to_string())).await.is_err() {
                break;
            }
        }
    }
}

async fn video_stream_task(mut video_rx: broadcast::Receiver<Bytes>, tx: mpsc::Sender<Message>) {
    loop {
        match video_rx.recv().await {
            Ok(data) => {
                if tx.send(Message::Binary(data.to_vec())).await.is_err() {
                    break;
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                warn!("video stream lagged by {n} frames");
            }
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}

fn http_request_with_bearer(url: &str, token: &str) -> anyhow::Result<http::Request<()>> {
    use tokio_tungstenite::tungstenite::handshake::client::generate_key;
    let parsed = url::Url::parse(url).map_err(|e| anyhow::anyhow!("invalid websocket url: {e}"))?;
    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("url has no host"))?
        .to_string();
    let host_header = match parsed.port() {
        Some(p) => format!("{host}:{p}"),
        None => host,
    };
    Ok(http::Request::builder()
        .uri(url)
        .header("Host", host_header)
        .header("Authorization", format!("Bearer {token}"))
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header("Sec-WebSocket-Key", generate_key())
        .body(())?)
}
