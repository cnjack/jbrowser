use std::collections::VecDeque;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    response::{IntoResponse, Response},
    Json,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tracing::info;
use uuid::Uuid;

use crate::db::repo;
use crate::server::auth::crypto::hash_token;
use crate::server::error::AppError;
use crate::server::state::{AppState, CdpTunnelEvent};

const CDP_TUNNEL_PENDING_LIMIT: usize = 256;

#[derive(Debug, Deserialize)]
pub struct CdpQuery {
    token: String,
}

pub async fn cdp_json_version(
    State(state): State<AppState>,
    Path((tenant_id, browser_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<CdpQuery>,
) -> Result<Json<Value>, AppError> {
    validate_cdp_token(&state, tenant_id, &query.token).await?;
    let browsers = state.browsers.read().await;
    let browser = browsers
        .get(&browser_id)
        .filter(|browser| browser.tenant_id == tenant_id)
        .ok_or_else(|| AppError::not_found("browser instance"))?;
    let target_id = browser
        .active_tab_id
        .clone()
        .unwrap_or_else(|| "browser".to_string());
    Ok(Json(json!({
        "Browser": format!("Chrome/{}", browser.browser_version),
        "Protocol-Version": "1.3",
        "User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome JBrowser",
        "V8-Version": "12.0.0",
        "WebKit-Version": "537.36",
        "webSocketDebuggerUrl": format!("{}/cdp/tenants/{}/browser-instances/{}/devtools/browser/{}?token={}", ws_base_url(&state.config.public_base_url), tenant_id, browser_id, target_id, query.token)
    })))
}

pub async fn cdp_json_list(
    State(state): State<AppState>,
    Path((tenant_id, browser_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<CdpQuery>,
) -> Result<Json<Value>, AppError> {
    validate_cdp_token(&state, tenant_id, &query.token).await?;
    let browsers = state.browsers.read().await;
    let browser = browsers
        .get(&browser_id)
        .filter(|browser| browser.tenant_id == tenant_id)
        .ok_or_else(|| AppError::not_found("browser instance"))?;
    let data = browser
        .tabs
        .iter()
        .map(|tab| {
            json!({
                "id": tab.id,
                "type": "page",
                "title": tab.title,
                "url": tab.url,
                "webSocketDebuggerUrl": format!("{}/cdp/tenants/{}/browser-instances/{}/devtools/page/{}?token={}", ws_base_url(&state.config.public_base_url), tenant_id, browser_id, tab.id, query.token),
                "devtoolsFrontendUrl": format!("/devtools/inspector.html?ws=/cdp/tenants/{}/browser-instances/{}/devtools/page/{}", tenant_id, browser_id, tab.id)
            })
        })
        .collect::<Vec<_>>();
    Ok(Json(json!(data)))
}

pub async fn cdp_ws_tunnel(
    State(state): State<AppState>,
    Path((tenant_id, browser_id, target_id)): Path<(Uuid, Uuid, String)>,
    Query(query): Query<CdpQuery>,
    ws: WebSocketUpgrade,
) -> Result<Response, AppError> {
    validate_cdp_token(&state, tenant_id, &query.token).await?;

    let agent_id = {
        let browsers = state.browsers.read().await;
        let browser = browsers
            .get(&browser_id)
            .filter(|b| b.tenant_id == tenant_id)
            .ok_or_else(|| AppError::not_found("browser instance"))?;
        browser.agent_id
    };

    {
        let senders = state.agent_senders.read().await;
        if !senders.contains_key(&agent_id) {
            return Err(AppError::bad_request("agent is not connected"));
        }
    }

    Ok(ws
        .on_upgrade(move |socket| handle_cdp_tunnel(state, agent_id, target_id, socket))
        .into_response())
}

async fn handle_cdp_tunnel(state: AppState, agent_id: Uuid, target_id: String, socket: WebSocket) {
    let session_id = Uuid::now_v7().to_string();
    info!(%session_id, %agent_id, %target_id, "CDP tunnel session opening");

    let (mut ws_tx, mut ws_rx) = socket.split();

    let (response_tx, mut response_rx) = mpsc::channel::<CdpTunnelEvent>(256);
    let mut cdp_ready = false;
    let mut pending_client_messages = VecDeque::<String>::new();

    state
        .cdp_tunnel_senders
        .write()
        .await
        .insert(session_id.clone(), response_tx);

    {
        let senders = state.agent_senders.read().await;
        if let Some(tx) = senders.get(&agent_id) {
            let open_msg = json!({
                "type": "cdp.tunnel.open",
                "payload": {
                    "session_id": session_id,
                    "target_id": target_id
                }
            });
            let _ = tx.try_send(open_msg.to_string());
        }
    }

    loop {
        tokio::select! {
            msg = ws_rx.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if cdp_ready {
                            if send_cdp_tunnel_message(&state, agent_id, &session_id, text).await.is_err() {
                                break;
                            }
                        } else {
                            if pending_client_messages.len() >= CDP_TUNNEL_PENDING_LIMIT {
                                tracing::warn!(
                                    %session_id,
                                    pending = pending_client_messages.len(),
                                    "CDP tunnel pending message buffer full before agent ready"
                                );
                                break;
                            }
                            pending_client_messages.push_back(text);
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
            Some(event) = response_rx.recv() => {
                match event {
                    CdpTunnelEvent::Ready => {
                        cdp_ready = true;
                        while let Some(text) = pending_client_messages.pop_front() {
                            if send_cdp_tunnel_message(&state, agent_id, &session_id, text).await.is_err() {
                                return close_cdp_tunnel(state, agent_id, session_id).await;
                            }
                        }
                    }
                    CdpTunnelEvent::Message(response) => {
                        if ws_tx.send(Message::Text(response)).await.is_err() {
                            break;
                        }
                    }
                }
            }
        }
    }

    close_cdp_tunnel(state, agent_id, session_id).await;
}

async fn send_cdp_tunnel_message(
    state: &AppState,
    agent_id: Uuid,
    session_id: &str,
    text: String,
) -> Result<(), ()> {
    let tx = {
        let senders = state.agent_senders.read().await;
        senders.get(&agent_id).cloned()
    };
    let Some(tx) = tx else {
        return Err(());
    };
    let tunnel_msg = json!({
        "type": "cdp.tunnel.message",
        "payload": {
            "session_id": session_id,
            "data": text
        }
    });
    tx.send(tunnel_msg.to_string()).await.map_err(|_| ())
}

async fn close_cdp_tunnel(state: AppState, agent_id: Uuid, session_id: String) {
    {
        let senders = state.agent_senders.read().await;
        if let Some(tx) = senders.get(&agent_id) {
            let close_msg = json!({
                "type": "cdp.tunnel.close",
                "payload": { "session_id": session_id }
            });
            let _ = tx.try_send(close_msg.to_string());
        }
    }

    state.cdp_tunnel_senders.write().await.remove(&session_id);
    info!(%session_id, "CDP tunnel session closed");
}

async fn validate_cdp_token(state: &AppState, tenant_id: Uuid, raw: &str) -> Result<(), AppError> {
    let hash = hash_token(raw);
    let token = repo::get_token_by_hash(&state.pool, &hash)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    match token {
        Some(t)
            if t.token_type == "tenant_cdp_access"
                && Uuid::parse_str(&t.tenant_id).ok() == Some(tenant_id) =>
        {
            Ok(())
        }
        _ => Err(AppError::unauthorized("invalid CDP token")),
    }
}

fn ws_base_url(public_base_url: &str) -> String {
    public_base_url
        .replace("https://", "wss://")
        .replace("http://", "ws://")
}
