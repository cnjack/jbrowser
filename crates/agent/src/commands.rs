use futures_util::SinkExt;
use jbrowser_shared::protocol::ControlToAgentMessage;
use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{info, warn};

use crate::globals::{input_tx, set_active_tab};

pub(crate) async fn dispatch_control_message(msg: ControlToAgentMessage) {
    if msg.kind == "input.event" {
        let evt = msg.payload["type"].as_str().unwrap_or("?");
        let x = msg.payload["x"].as_f64().unwrap_or(-1.0);
        let y = msg.payload["y"].as_f64().unwrap_or(-1.0);
        info!("input.event type={evt} x={x:.0} y={y:.0}");
    } else {
        info!("control message: {}", msg.kind);
    }
    if msg.kind == "browser.reset" {
        match handle_browser_reset(&msg.payload).await {
            Ok(()) => {}
            Err(e) => {
                warn!("browser.reset failed: {e}");
                crate::globals::send_to_control(Message::Text(
                    json!({"type": "reset.failed", "payload": {"error": e.to_string()}})
                        .to_string(),
                ))
                .await;
            }
        }
        return;
    }
    let result = match msg.kind.as_str() {
        "navigate.url" => handle_navigate(&msg.payload).await,
        "navigate.back" => handle_navigate_history("Page.goBack").await,
        "navigate.forward" => handle_navigate_history("Page.goForward").await,
        "navigate.reload" => handle_navigate_history("Page.reload").await,
        "input.event" => handle_input_event(&msg.payload).await,
        "tab.command" => handle_tab_command(&msg.payload).await,
        "cdp.tunnel.open" => {
            crate::cdp_tunnel::handle_cdp_tunnel_open(&msg.payload).await;
            Ok(())
        }
        "cdp.tunnel.message" => {
            crate::cdp_tunnel::handle_cdp_tunnel_message(&msg.payload).await;
            Ok(())
        }
        "cdp.tunnel.close" => {
            crate::cdp_tunnel::handle_cdp_tunnel_close(&msg.payload).await;
            Ok(())
        }
        other => {
            info!("unhandled control message kind: {other}");
            Ok(())
        }
    };
    if let Err(e) = result {
        warn!("control message '{}' error: {e}", msg.kind);
    }
}

async fn handle_navigate(payload: &serde_json::Value) -> anyhow::Result<()> {
    let url = payload["url"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("navigate.url: missing url"))?;
    if let Some(tx) = input_tx() {
        let _ = tx.try_send(json!({"type": "navigate.url", "url": url}));
    }
    Ok(())
}

async fn handle_navigate_history(method: &str) -> anyhow::Result<()> {
    let event_type = match method {
        "Page.goBack" => "navigate.back",
        "Page.goForward" => "navigate.forward",
        "Page.reload" => "navigate.reload",
        _ => return Ok(()),
    };
    if let Some(tx) = input_tx() {
        let _ = tx.try_send(json!({"type": event_type}));
    }
    Ok(())
}

async fn handle_input_event(payload: &serde_json::Value) -> anyhow::Result<()> {
    if let Some(tx) = input_tx() {
        let _ = tx.try_send(payload.clone());
    }
    Ok(())
}

async fn handle_tab_command(payload: &serde_json::Value) -> anyhow::Result<()> {
    info!("handle_tab_command: {:?}", payload);
    let client = reqwest::Client::new();
    match payload["command"].as_str().unwrap_or("") {
        "new" | "open" => {
            let url = payload["url"].as_str().unwrap_or("about:blank");
            let resp_text = client
                .put(format!("http://localhost:9222/json/new?{url}"))
                .send()
                .await?
                .text()
                .await
                .unwrap_or_default();
            info!(
                "json/new response: {:?}",
                &resp_text[..resp_text.len().min(200)]
            );
            if url != "about:blank" {
                let resp: serde_json::Value =
                    serde_json::from_str(&resp_text).unwrap_or(serde_json::Value::Null);
                if let Some(ws_url) = resp["webSocketDebuggerUrl"].as_str() {
                    if let Ok((mut tab_ws, _)) = connect_async(ws_url).await {
                        let _ = tab_ws
                            .send(Message::Text(
                                json!({"id":1,"method":"Page.navigate","params":{"url":url}})
                                    .to_string(),
                            ))
                            .await;
                        tab_ws.close(None).await.ok();
                    }
                }
            }
        }
        "close" => {
            if let Some(tab_id) = payload["tabId"].as_str() {
                client
                    .post(format!("http://localhost:9222/json/close/{tab_id}"))
                    .send()
                    .await?
                    .error_for_status()?;
            }
        }
        "activate" => {
            if let Some(tab_id) = payload["tabId"].as_str() {
                match client
                    .get(format!("http://localhost:9222/json/activate/{tab_id}"))
                    .send()
                    .await
                {
                    Ok(resp) => info!("json/activate/{tab_id} → {}", resp.status()),
                    Err(e) => warn!("json/activate/{tab_id} failed: {e}"),
                }
                info!("set_active_tab → {tab_id}");
                set_active_tab(tab_id);
            }
        }
        other => warn!("unknown tab command: {other}"),
    }
    Ok(())
}

async fn handle_browser_reset(_payload: &serde_json::Value) -> anyhow::Result<()> {
    use futures_util::StreamExt as _;
    use std::collections::HashSet;

    let client = reqwest::Client::new();

    let targets: Vec<serde_json::Value> = client
        .get("http://localhost:9222/json/list")
        .send()
        .await?
        .json()
        .await?;

    let pages: Vec<_> = targets
        .iter()
        .filter(|t| t["type"].as_str() == Some("page"))
        .collect();

    // Close all tabs except the first one
    for tab in pages.iter().skip(1) {
        if let Some(id) = tab["id"].as_str() {
            let _ = client
                .post(format!("http://localhost:9222/json/close/{id}"))
                .send()
                .await;
        }
    }

    // Ensure at least one page exists
    if pages.is_empty() {
        client
            .put("http://localhost:9222/json/new?about:blank")
            .send()
            .await
            .ok();
    } else if let Some(tab) = pages.first() {
        if let Some(ws_url) = tab["webSocketDebuggerUrl"].as_str() {
            let (mut ws, _) = connect_async(ws_url).await?;
            // Send 3 CDP commands
            ws.send(Message::Text(
                json!({"id":1,"method":"Page.navigate","params":{"url":"about:blank"}}).to_string(),
            ))
            .await?;
            ws.send(Message::Text(
                json!({"id":2,"method":"Network.clearBrowserCookies","params":{}}).to_string(),
            ))
            .await?;
            ws.send(Message::Text(
                json!({"id":3,"method":"Network.clearBrowserCache","params":{}}).to_string(),
            ))
            .await?;

            // Wait for all 3 responses or 5s timeout
            let mut received: HashSet<u64> = HashSet::new();
            let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(5);
            loop {
                let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                if remaining.is_zero() {
                    break;
                }
                match tokio::time::timeout(remaining, ws.next()).await {
                    Ok(Some(Ok(Message::Text(text)))) => {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                            if let Some(id) = v["id"].as_u64() {
                                received.insert(id);
                                if received.contains(&1) && received.contains(&2) && received.contains(&3) {
                                    break;
                                }
                            }
                        }
                    }
                    Ok(Some(Ok(_))) => continue,
                    _ => break,
                }
            }
            ws.close(None).await.ok();
        }
    }

    // Fetch updated tab list
    let new_targets: Vec<serde_json::Value> = client
        .get("http://localhost:9222/json/list")
        .send()
        .await?
        .json()
        .await?;
    let tabs: Vec<serde_json::Value> = new_targets
        .iter()
        .filter(|t| t["type"].as_str() == Some("page"))
        .enumerate()
        .map(|(i, t)| {
            json!({
                "id": t["id"],
                "title": t["title"],
                "url": t["url"],
                "active": i == 0,
            })
        })
        .collect();

    info!("browser reset complete, {} tabs", tabs.len());
    crate::globals::send_to_control(Message::Text(
        json!({"type": "reset.completed", "payload": {"tabs": tabs}}).to_string(),
    ))
    .await;

    Ok(())
}
