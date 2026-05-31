use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::HeaderMap,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::sync::{broadcast, mpsc};
use tracing::info;
use uuid::Uuid;

use jbrowser_shared::{
    constants::{DEFAULT_VIEWPORT_HEIGHT, DEFAULT_VIEWPORT_WIDTH},
    models::{AgentStatus, AgentSummary, BrowserInstance, BrowserStatus, BrowserTab},
    protocol::decode_video_frame,
};

use crate::db::repo;
use crate::server::auth::crypto::hash_token;
use crate::server::auth::middleware::bearer_token;
use crate::server::error::AppError;
use crate::server::state::AppState;

pub async fn ws_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<Response, AppError> {
    let token =
        bearer_token(&headers).ok_or_else(|| AppError::unauthorized("missing agent token"))?;
    let token_hash = hash_token(token);
    let token_row = repo::get_token_by_hash(&state.pool, &token_hash)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?
        .ok_or_else(|| AppError::unauthorized("invalid agent token"))?;
    if token_row.token_type != "agent_runtime" {
        return Err(AppError::unauthorized("invalid agent token"));
    }
    let tenant_id = Uuid::parse_str(&token_row.tenant_id).unwrap();
    let agent_id_from_token = token_row
        .name
        .as_deref()
        .and_then(|n| n.strip_prefix("runtime-"))
        .and_then(|s| Uuid::parse_str(s).ok());
    Ok(ws
        .on_upgrade(move |socket| {
            handle_agent_socket(state, tenant_id, agent_id_from_token, socket)
        })
        .into_response())
}

async fn handle_agent_socket(
    state: AppState,
    tenant_id: Uuid,
    agent_id_hint: Option<Uuid>,
    socket: WebSocket,
) {
    info!(%tenant_id, "agent websocket connected");

    let (agent_id, browser_id) = {
        let agents = state.agents.read().await;
        let found = agent_id_hint
            .and_then(|id| agents.get(&id).cloned())
            .or_else(|| agents.values().find(|a| a.tenant_id == tenant_id).cloned());
        drop(agents);

        if let Some(a) = found {
            (a.id, a.browser_instance_id)
        } else if let Some(hint_id) = agent_id_hint {
            match repo::get_agent(&state.pool, tenant_id, hint_id).await {
                Ok(Some(row)) => {
                    let bid = Uuid::parse_str(&row.browser_instance_id)
                        .unwrap_or_else(|_| Uuid::now_v7());
                    let agent_name = row
                        .name
                        .clone()
                        .unwrap_or_else(|| format!("agent-{hint_id}"));
                    let browser_row = repo::get_browser_instance(&state.pool, tenant_id, bid)
                        .await
                        .ok()
                        .flatten();
                    let (btype, bver) = browser_row
                        .as_ref()
                        .map(|b| {
                            (
                                b.browser_type.clone(),
                                b.browser_version
                                    .clone()
                                    .unwrap_or_else(|| "unknown".to_string()),
                            )
                        })
                        .unwrap_or_else(|| ("chromium".to_string(), "unknown".to_string()));
                    let agent = AgentSummary {
                        id: hint_id,
                        tenant_id,
                        browser_instance_id: bid,
                        name: agent_name.clone(),
                        status: AgentStatus::Offline,
                        browser_type: btype.clone(),
                        browser_version: bver.clone(),
                        last_heartbeat_at: None,
                    };
                    let browser = BrowserInstance {
                        id: bid,
                        tenant_id,
                        agent_id: hint_id,
                        name: format!("{btype}-{bid}"),
                        status: BrowserStatus::Offline,
                        browser_type: btype.clone(),
                        browser_version: bver.clone(),
                        active_tab_id: None,
                        tabs: Vec::new(),
                        proxy_enabled: false,
                        viewport_width: DEFAULT_VIEWPORT_WIDTH,
                        viewport_height: DEFAULT_VIEWPORT_HEIGHT,
                        viewer_count: 0,
                        agent_name,
                        agent_status: AgentStatus::Offline,
                        last_heartbeat_at: None,
                    };
                    state.agents.write().await.insert(hint_id, agent);
                    state.browsers.write().await.insert(bid, browser);
                    info!(%hint_id, "agent restored from DB into memory");
                    (hint_id, bid)
                }
                _ => {
                    info!(%tenant_id, %hint_id, "agent not found in memory or DB, closing socket");
                    return;
                }
            }
        } else {
            info!(%tenant_id, "no agent found for tenant, closing socket");
            return;
        }
    };

    let (bcast_tx, _) = broadcast::channel::<Vec<u8>>(16);
    state
        .browser_preview
        .write()
        .await
        .insert(browser_id, bcast_tx.clone());

    let (event_tx, _) = broadcast::channel::<String>(64);
    state
        .browser_events
        .write()
        .await
        .insert(browser_id, event_tx.clone());

    let (cmd_tx, mut cmd_rx) = mpsc::channel::<String>(64);
    state.agent_senders.write().await.insert(agent_id, cmd_tx);

    let (mut ws_tx, mut ws_rx) = socket.split();

    loop {
        tokio::select! {
            msg = ws_rx.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(value) = serde_json::from_str::<Value>(&text) {
                            let msg_type = value.get("type").and_then(Value::as_str);

                            if msg_type == Some("cdp.tunnel.message") {
                                if let Some(payload) = value.get("payload") {
                                    if let (Some(session_id), Some(data)) = (
                                        payload.get("session_id").and_then(Value::as_str),
                                        payload.get("data").and_then(Value::as_str),
                                    ) {
                                        let senders = state.cdp_tunnel_senders.read().await;
                                        if let Some(tx) = senders.get(session_id) {
                                            let _ = tx.try_send(data.to_string());
                                        }
                                    }
                                }
                            }

                            if msg_type == Some("tab.list") {
                                let _ = event_tx.send(text.clone());
                            }
                            handle_agent_text(&state, agent_id, browser_id, value).await;
                        }
                    }
                    Some(Ok(Message::Binary(bytes))) => {
                        if let Ok(_frame) = decode_video_frame(bytes.clone().into()) {
                            let subs = bcast_tx.receiver_count();
                            let _ = bcast_tx.send(bytes.clone());
                            state.browser_last_frame.write().await.insert(browser_id, bytes);
                            tracing::debug!(seq = _frame.sequence, subs, "video frame relayed");
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
            Some(cmd) = cmd_rx.recv() => {
                if ws_tx.send(Message::Text(cmd)).await.is_err() {
                    break;
                }
            }
        }
    }

    state.agent_senders.write().await.remove(&agent_id);
    state.browser_preview.write().await.remove(&browser_id);
    state.browser_last_frame.write().await.remove(&browser_id);
    state.browser_events.write().await.remove(&browser_id);
    {
        let mut agents = state.agents.write().await;
        if let Some(a) = agents.get_mut(&agent_id) {
            a.status = AgentStatus::Offline;
        }
    }
    {
        let mut browsers = state.browsers.write().await;
        if let Some(b) = browsers.get_mut(&browser_id) {
            b.status = BrowserStatus::Offline;
            b.agent_status = AgentStatus::Offline;
        }
    }
    info!(%tenant_id, %agent_id, "agent websocket disconnected");
}

async fn handle_agent_text(state: &AppState, agent_id: Uuid, browser_id: Uuid, value: Value) {
    let now = Utc::now().to_rfc3339();
    let msg_type = value.get("type").and_then(Value::as_str).unwrap_or("");

    // Update agent heartbeat unconditionally
    {
        let mut agents = state.agents.write().await;
        if let Some(agent) = agents.get_mut(&agent_id) {
            agent.status = AgentStatus::Online;
            agent.last_heartbeat_at = Some(now.clone());
        }
    }

    match msg_type {
        "reset.completed" => {
            state.pending_resets.write().await.remove(&browser_id);
            {
                let mut browsers = state.browsers.write().await;
                if let Some(browser) = browsers.get_mut(&browser_id) {
                    browser.status = BrowserStatus::Online;
                    browser.agent_status = AgentStatus::Online;
                    browser.last_heartbeat_at = Some(now);
                    if let Some(tabs) = value.pointer("/payload/tabs") {
                        if let Ok(parsed) = serde_json::from_value::<Vec<BrowserTab>>(tabs.clone()) {
                            browser.active_tab_id =
                                parsed.iter().find(|t| t.active).map(|t| t.id.clone());
                            browser.tabs = parsed;
                        }
                    }
                }
            }
            // Broadcast to frontend viewers
            let ev_map = state.browser_events.read().await;
            if let Some(tx) = ev_map.get(&browser_id) {
                let _ = tx.send(
                    serde_json::to_string(&serde_json::json!({"type":"reset.completed","payload":{}}))
                        .unwrap_or_default(),
                );
            }
            info!(%browser_id, "reset.completed received");
        }
        "reset.failed" => {
            state.pending_resets.write().await.remove(&browser_id);
            {
                let mut browsers = state.browsers.write().await;
                if let Some(browser) = browsers.get_mut(&browser_id) {
                    browser.status = BrowserStatus::Online;
                    browser.agent_status = AgentStatus::Online;
                    browser.last_heartbeat_at = Some(now);
                }
            }
            let error = value
                .pointer("/payload/error")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let ev_map = state.browser_events.read().await;
            if let Some(tx) = ev_map.get(&browser_id) {
                let _ = tx.send(
                    serde_json::to_string(&serde_json::json!({"type":"reset.failed","payload":{"error":error}}))
                        .unwrap_or_default(),
                );
            }
            info!(%browser_id, %error, "reset.failed received");
        }
        _ => {
            // While a reset is pending, don't overwrite status back to Online
            let is_resetting = state.pending_resets.read().await.contains_key(&browser_id);
            let mut browsers = state.browsers.write().await;
            if let Some(browser) = browsers.get_mut(&browser_id) {
                if !is_resetting {
                    browser.status = BrowserStatus::Online;
                }
                browser.agent_status = AgentStatus::Online;
                browser.last_heartbeat_at = Some(now);
                if msg_type == "tab.list" {
                    if let Some(tabs) =
                        value.get("payload").and_then(|payload| payload.get("tabs"))
                    {
                        if !is_resetting {
                            if let Ok(parsed) =
                                serde_json::from_value::<Vec<BrowserTab>>(tabs.clone())
                            {
                                browser.active_tab_id = parsed
                                    .iter()
                                    .find(|tab| tab.active)
                                    .map(|tab| tab.id.clone());
                                browser.tabs = parsed;
                            }
                        }
                    }
                }
            }
        }
    }
}
