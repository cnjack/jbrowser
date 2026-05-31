use std::time::Duration;

use anyhow::Context;
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{info, warn};

use crate::chrome::ensure_page_target;
use crate::globals::active_tab_rx;

pub(crate) async fn input_loop(mut rx: mpsc::Receiver<serde_json::Value>) {
    loop {
        match run_input_session(&mut rx).await {
            Ok(()) => {
                info!("input CDP session ended, reconnecting in 100ms");
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Err(e) => {
                warn!("input CDP session error: {e}, reconnecting in 500ms");
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
    }
}

async fn run_input_session(rx: &mut mpsc::Receiver<serde_json::Value>) -> anyhow::Result<()> {
    let target = ensure_page_target().await?;
    let ws_url = target["webSocketDebuggerUrl"]
        .as_str()
        .context("target has no webSocketDebuggerUrl")?;
    let (ws, _) = connect_async(ws_url)
        .await
        .context("input CDP WS connect failed")?;
    let (mut write, mut read) = ws.split();
    info!("input CDP session connected");

    write
        .send(Message::Text(
            json!({
                "id": 999,
                "method": "Emulation.setDeviceMetricsOverride",
                "params": {
                    "width": 1280,
                    "height": 720,
                    "deviceScaleFactor": 1,
                    "mobile": false
                }
            })
            .to_string(),
        ))
        .await?;

    let mut cmd_id: u64 = 1000;
    let mut tab_rx = active_tab_rx();
    loop {
        tokio::select! {
            msg = rx.recv() => {
                let Some(payload) = msg else { return Ok(()); };
                let commands = build_input_cdp_commands(&mut cmd_id, &payload);
                if !commands.is_empty() {
                    info!("input CDP sending {} command(s) for type={}", commands.len(), payload["type"].as_str().unwrap_or("?"));
                }
                for cmd in commands {
                    write.send(Message::Text(cmd)).await?;
                }
            }
            item = read.next() => {
                match item {
                    None => return Ok(()),
                    Some(Err(e)) => return Err(e.into()),
                    Some(Ok(_)) => {}
                }
            }
            _ = tab_rx.changed() => {
                info!("active tab changed, reconnecting input session");
                return Ok(());
            }
        }
    }
}

pub(crate) fn build_input_cdp_commands(
    cmd_id: &mut u64,
    payload: &serde_json::Value,
) -> Vec<String> {
    let event_type = payload["type"].as_str().unwrap_or("");
    let x = payload["x"].as_f64().unwrap_or(0.0);
    let y = payload["y"].as_f64().unwrap_or(0.0);
    let mut cmds = Vec::new();

    match event_type {
        "navigate.url" => {
            let url = payload["url"].as_str().unwrap_or("about:blank");
            cmds.push(
                json!({"id": *cmd_id, "method": "Page.navigate", "params": {"url": url}})
                    .to_string(),
            );
            *cmd_id += 1;
        }
        "navigate.back" => {
            cmds.push(json!({"id": *cmd_id, "method": "Page.goBack", "params": {}}).to_string());
            *cmd_id += 1;
        }
        "navigate.forward" => {
            cmds.push(json!({"id": *cmd_id, "method": "Page.goForward", "params": {}}).to_string());
            *cmd_id += 1;
        }
        "navigate.reload" => {
            cmds.push(json!({"id": *cmd_id, "method": "Page.reload", "params": {}}).to_string());
            *cmd_id += 1;
        }
        "click" => {
            let button = payload["button"].as_str().unwrap_or("left");
            let modifiers = payload["modifiers"].as_i64().unwrap_or(0);
            cmds.push(json!({"id": *cmd_id, "method": "Input.dispatchMouseEvent",
                "params": {"type": "mousePressed", "x": x, "y": y, "button": button, "clickCount": 1, "modifiers": modifiers}}).to_string());
            *cmd_id += 1;
            cmds.push(json!({"id": *cmd_id, "method": "Input.dispatchMouseEvent",
                "params": {"type": "mouseReleased", "x": x, "y": y, "button": button, "clickCount": 1, "modifiers": modifiers}}).to_string());
            *cmd_id += 1;
        }
        "mousedown" => {
            let button = payload["button"].as_str().unwrap_or("left");
            let click_count = payload["clickCount"].as_i64().unwrap_or(1);
            let modifiers = payload["modifiers"].as_i64().unwrap_or(0);
            cmds.push(json!({"id": *cmd_id, "method": "Input.dispatchMouseEvent",
                "params": {"type": "mousePressed", "x": x, "y": y, "button": button, "clickCount": click_count, "modifiers": modifiers}}).to_string());
            *cmd_id += 1;
        }
        "mouseup" => {
            let button = payload["button"].as_str().unwrap_or("left");
            let click_count = payload["clickCount"].as_i64().unwrap_or(1);
            let modifiers = payload["modifiers"].as_i64().unwrap_or(0);
            cmds.push(json!({"id": *cmd_id, "method": "Input.dispatchMouseEvent",
                "params": {"type": "mouseReleased", "x": x, "y": y, "button": button, "clickCount": click_count, "modifiers": modifiers}}).to_string());
            *cmd_id += 1;
        }
        "mousemove" => {
            let modifiers = payload["modifiers"].as_i64().unwrap_or(0);
            cmds.push(
                json!({"id": *cmd_id, "method": "Input.dispatchMouseEvent",
                "params": {"type": "mouseMoved", "x": x, "y": y, "modifiers": modifiers}})
                .to_string(),
            );
            *cmd_id += 1;
        }
        "wheel" => {
            let dx = payload["deltaX"].as_f64().unwrap_or(0.0);
            let dy = payload["deltaY"].as_f64().unwrap_or(0.0);
            cmds.push(
                json!({"id": *cmd_id, "method": "Input.dispatchMouseEvent",
                "params": {"type": "mouseWheel", "x": x, "y": y, "deltaX": dx, "deltaY": dy}})
                .to_string(),
            );
            *cmd_id += 1;
        }
        "keydown" | "keyup" => {
            let key = payload["key"].as_str().unwrap_or("");
            let code = payload["code"].as_str().unwrap_or("");
            let text = payload["text"].as_str().unwrap_or("");
            let modifiers = payload["modifiers"].as_i64().unwrap_or(0);
            let key_code = payload["keyCode"].as_i64().unwrap_or(0);
            let cdp_type = if event_type == "keydown" {
                "keyDown"
            } else {
                "keyUp"
            };
            cmds.push(
                json!({"id": *cmd_id, "method": "Input.dispatchKeyEvent",
                "params": {"type": cdp_type, "key": key, "code": code, "text": text,
                           "modifiers": modifiers, "windowsVirtualKeyCode": key_code,
                           "nativeVirtualKeyCode": key_code}})
                .to_string(),
            );
            *cmd_id += 1;
        }
        other => warn!("unknown input event type: {other}"),
    }
    cmds
}
