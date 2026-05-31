use std::time::Duration;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::{IntoResponse, Response},
};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::broadcast;
use uuid::Uuid;

use jbrowser_shared::protocol::{ClientMessage, ServerMessage};

use crate::db::repo;
use crate::server::auth::crypto::decode_jwt;
use crate::server::state::AppState;
use crate::server::types::JwtClaims;

pub async fn ws_control(State(state): State<AppState>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |socket| handle_control_socket(state, socket))
        .into_response()
}

async fn handle_control_socket(state: AppState, socket: WebSocket) {
    tracing::debug!("control WS connection opened");
    let (mut sender, mut receiver) = socket.split();
    let auth_timeout = tokio::time::sleep(Duration::from_secs(10));
    tokio::pin!(auth_timeout);

    let claims = loop {
        tokio::select! {
            _ = &mut auth_timeout => {
                let _ = send_json(&mut sender, ServerMessage::new("auth.error", json!({"message": "auth timeout"}))).await;
                return;
            }
            Some(Ok(message)) = receiver.next() => {
                match message {
                    Message::Text(text) => {
                        match serde_json::from_str::<ClientMessage>(&text) {
                            Ok(message) if message.kind == "auth" => {
                                let Some(token) = message.payload.get("token").and_then(Value::as_str) else {
                                    let _ = send_json(&mut sender, ServerMessage::new("auth.error", json!({"message": "missing token"}))).await;
                                    return;
                                };
                                match decode_jwt(&state.config.jwt_secret, token) {
                                    Ok(claims) => {
                                        let _ = send_json(&mut sender, ServerMessage::new("auth.ok", json!({}))).await;
                                        tracing::debug!(sub = %claims.sub, "control WS authenticated");
                                        break claims;
                                    }
                                    Err(_) => {
                                        let _ = send_json(&mut sender, ServerMessage::new("auth.error", json!({"message": "invalid token"}))).await;
                                        return;
                                    }
                                }
                            }
                            Ok(message) if message.kind == "ping" => {
                                let _ = send_json(&mut sender, ServerMessage::new("pong", json!({}))).await;
                            }
                            _ => {
                                let _ = send_json(&mut sender, ServerMessage::new("error", json!({"message": "authenticate first"}))).await;
                            }
                        }
                    }
                    Message::Close(_) => return,
                    _ => {}
                }
            }
            else => return,
        }
    };

    let mut current_browser_id: Option<Uuid> = None;
    let mut preview_rx: Option<broadcast::Receiver<Vec<u8>>> = None;
    let mut events_rx: Option<broadcast::Receiver<String>> = None;

    loop {
        let video_fut = async {
            match preview_rx.as_mut() {
                Some(rx) => rx.recv().await.ok(),
                None => std::future::pending().await,
            }
        };

        let event_fut = async {
            match events_rx.as_mut() {
                Some(rx) => rx.recv().await.ok(),
                None => std::future::pending().await,
            }
        };

        tokio::select! {
            Some(Ok(message)) = receiver.next() => {
                if let Message::Text(text) = message {
                    if let Ok(parsed) = serde_json::from_str::<ClientMessage>(&text) {
                        if parsed.kind == "browser.subscribe" {
                            tracing::debug!(payload = %parsed.payload, "browser.subscribe received");
                            if let Some(bid) = parsed.payload.get("browserInstanceId")
                                .and_then(Value::as_str)
                                .and_then(|id| Uuid::parse_str(id).ok())
                            {
                                current_browser_id = Some(bid);
                                let map = state.browser_preview.read().await;
                                let found = map.contains_key(&bid);
                                tracing::debug!(%bid, found, keys = ?map.keys().collect::<Vec<_>>(), "control client subscribing to browser preview");
                                preview_rx = map.get(&bid).map(|tx| tx.subscribe());
                                let ev_map = state.browser_events.read().await;
                                events_rx = ev_map.get(&bid).map(|tx| tx.subscribe());
                                let last = state.browser_last_frame.read().await;
                                if let Some(frame) = last.get(&bid) {
                                    let _ = sender.send(Message::Binary(frame.clone())).await;
                                }
                            }
                        }
                    }
                    handle_control_text(&state, &claims, &mut sender, &text, current_browser_id).await;
                }
            }
            Some(event_json) = event_fut => {
                if sender.send(Message::Text(event_json)).await.is_err() {
                    return;
                }
            }
            Some(bytes) = video_fut => {
                if sender.send(Message::Binary(bytes)).await.is_err() {
                    return;
                }
                if let Some(bid) = current_browser_id {
                    if preview_rx.as_mut().map(|rx| rx.is_empty()).unwrap_or(false) {
                        // still valid
                    } else {
                        let map = state.browser_preview.read().await;
                        if let Some(tx) = map.get(&bid) {
                            preview_rx = Some(tx.subscribe());
                        }
                    }
                }
            }
            else => return,
        }
    }
}

async fn handle_control_text(
    state: &AppState,
    claims: &JwtClaims,
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    text: &str,
    current_browser_id: Option<Uuid>,
) {
    let Ok(message) = serde_json::from_str::<ClientMessage>(text) else {
        let _ = send_json(
            sender,
            ServerMessage::new("error", json!({"message": "invalid json"})),
        )
        .await;
        return;
    };

    match message.kind.as_str() {
        "ping" => {
            let _ = send_json(sender, ServerMessage::new("pong", json!({}))).await;
        }
        "browser.subscribe" => {
            let Some(browser_id) = message
                .payload
                .get("browserInstanceId")
                .and_then(Value::as_str)
                .and_then(|id| Uuid::parse_str(id).ok())
            else {
                let _ = send_json(
                    sender,
                    ServerMessage::new("error", json!({"message": "missing browserInstanceId"})),
                )
                .await;
                return;
            };
            let browsers = state.browsers.read().await;
            if let Some(browser) = browsers.get(&browser_id).filter(|browser| {
                claims
                    .tenants
                    .iter()
                    .any(|tenant| tenant.id == browser.tenant_id)
            }) {
                let _ =
                    send_json(sender, ServerMessage::new("browser.state", json!(browser))).await;
                let _ = send_json(
                    sender,
                    ServerMessage::new("tab.list", json!({ "tabs": browser.tabs })),
                )
                .await;
            }
        }
        "input.event" | "tab.command" | "navigate.url" | "navigate.back"
        | "navigate.forward" | "navigate.reload" => {
            let browser_id = message
                .payload
                .get("browserInstanceId")
                .and_then(Value::as_str)
                .and_then(|id| Uuid::parse_str(id).ok())
                .or(current_browser_id);

            if let Some(tenant) = claims.tenants.first() {
                let _ = repo::create_audit_log(
                    &state.pool,
                    &repo::CreateAuditLog {
                        id: Uuid::now_v7(),
                        tenant_id: tenant.id,
                        actor_type: "user",
                        actor_id: &claims.sub,
                        action: match message.kind.as_str() {
                            "input.event" => "input.dispatched",
                            "tab.command" => "tab.command_requested",
                            _ => "navigate.requested",
                        },
                        source: "web_ui",
                        resource_type: None,
                        resource_id: None,
                        browser_instance_id: None,
                        tab_id: None,
                        metadata: None,
                    },
                )
                .await;
            }

            if let Some(bid) = browser_id {
                let browsers = state.browsers.read().await;
                if let Some(browser) = browsers.get(&bid) {
                    let agent_id = browser.agent_id;
                    drop(browsers);
                    let senders = state.agent_senders.read().await;
                    if let Some(tx) = senders.get(&agent_id) {
                        let cmd = serde_json::to_string(&message).unwrap_or_default();
                        let _ = tx.try_send(cmd);
                    }
                }
            }

            let _ = send_json(
                sender,
                ServerMessage::new("browser.state", json!({"accepted": message.kind})),
            )
            .await;
        }
        _ => {
            let _ = send_json(
                sender,
                ServerMessage::new("error", json!({"message": "unsupported message"})),
            )
            .await;
        }
    }
}

pub async fn send_json(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    message: ServerMessage,
) -> Result<(), axum::Error> {
    sender
        .send(Message::Text(
            serde_json::to_string(&message).expect("server message serialization"),
        ))
        .await
}
