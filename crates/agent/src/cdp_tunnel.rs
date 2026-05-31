use anyhow::Context;
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{info, warn};

use crate::chrome::cdp_get_targets;
use crate::globals::{cdp_tunnels, send_to_control};

pub(crate) async fn handle_cdp_tunnel_open(payload: &serde_json::Value) {
    let session_id = match payload["session_id"].as_str() {
        Some(s) => s.to_string(),
        None => {
            warn!("cdp.tunnel.open: missing session_id");
            return;
        }
    };
    let target_id = match payload["target_id"].as_str() {
        Some(s) => s.to_string(),
        None => {
            warn!("cdp.tunnel.open: missing target_id");
            return;
        }
    };

    info!(%session_id, %target_id, "opening CDP tunnel session");

    let targets = match cdp_get_targets().await {
        Ok(t) => t,
        Err(e) => {
            warn!("cdp.tunnel.open: failed to get targets: {e}");
            return;
        }
    };

    let ws_url = targets
        .iter()
        .find(|t| t["id"].as_str() == Some(&target_id))
        .and_then(|t| t["webSocketDebuggerUrl"].as_str())
        .map(String::from);

    let ws_url = match ws_url {
        Some(u) => u,
        None => {
            warn!(%session_id, %target_id, "cdp.tunnel.open: target not found");
            return;
        }
    };

    let (tunnel_tx, mut tunnel_rx) = mpsc::channel::<String>(256);

    cdp_tunnels()
        .write()
        .await
        .insert(session_id.clone(), tunnel_tx);

    let sid = session_id.clone();
    tokio::spawn(async move {
        let result = run_cdp_tunnel_session(&sid, &ws_url, &mut tunnel_rx).await;
        if let Err(e) = result {
            warn!(session_id = %sid, "CDP tunnel session error: {e}");
        }
        cdp_tunnels().write().await.remove(&sid);
        info!(session_id = %sid, "CDP tunnel session ended");
    });
}

async fn run_cdp_tunnel_session(
    session_id: &str,
    ws_url: &str,
    tunnel_rx: &mut mpsc::Receiver<String>,
) -> anyhow::Result<()> {
    let (cdp_ws, _) = connect_async(ws_url)
        .await
        .context("CDP tunnel WS connect failed")?;
    let (mut cdp_write, mut cdp_read) = cdp_ws.split();

    info!(%session_id, "CDP tunnel connected to Chrome");

    loop {
        tokio::select! {
            msg = tunnel_rx.recv() => {
                match msg {
                    Some(text) => {
                        if cdp_write.send(Message::Text(text)).await.is_err() {
                            break;
                        }
                    }
                    None => break,
                }
            }
            msg = cdp_read.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let response = json!({
                            "type": "cdp.tunnel.message",
                            "payload": {
                                "session_id": session_id,
                                "data": text
                            }
                        });
                        if !send_to_control(
                            tokio_tungstenite::tungstenite::Message::Text(response.to_string())
                        ).await {
                            warn!(%session_id, "failed to send CDP tunnel response to control plane");
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(e)) => {
                        warn!(%session_id, "CDP tunnel Chrome WS error: {e}");
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

pub(crate) async fn handle_cdp_tunnel_message(payload: &serde_json::Value) {
    let session_id = match payload["session_id"].as_str() {
        Some(s) => s,
        None => return,
    };
    let data = match payload["data"].as_str() {
        Some(d) => d.to_string(),
        None => return,
    };

    let tunnels = cdp_tunnels().read().await;
    if let Some(tx) = tunnels.get(session_id) {
        if tx.try_send(data).is_err() {
            warn!(%session_id, "CDP tunnel message send failed (channel full or closed)");
        }
    } else {
        warn!(%session_id, "CDP tunnel session not found");
    }
}

pub(crate) async fn handle_cdp_tunnel_close(payload: &serde_json::Value) {
    let session_id = match payload["session_id"].as_str() {
        Some(s) => s,
        None => return,
    };
    info!(%session_id, "closing CDP tunnel session");
    cdp_tunnels().write().await.remove(session_id);
}
