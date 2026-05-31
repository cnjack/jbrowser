use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde_json::{json, Value};
use std::time::Instant;
use uuid::Uuid;

use jbrowser_shared::models::BrowserStatus;

use crate::db::repo;
use crate::server::auth::middleware::authorize;
use crate::server::error::AppError;
use crate::server::state::AppState;

pub async fn list_browsers(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    authorize(&state, &headers, Some(tenant_id)).await?;
    let browsers = state.browsers.read().await;
    let data: Vec<_> = browsers
        .values()
        .filter(|browser| browser.tenant_id == tenant_id)
        .cloned()
        .collect();
    Ok(Json(json!({ "data": data })))
}

pub async fn get_browser(
    State(state): State<AppState>,
    Path((tenant_id, browser_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    authorize(&state, &headers, Some(tenant_id)).await?;
    let browsers = state.browsers.read().await;
    let browser = browsers
        .get(&browser_id)
        .filter(|browser| browser.tenant_id == tenant_id)
        .cloned()
        .ok_or_else(|| AppError::not_found("browser instance"))?;
    Ok(Json(json!({ "data": browser })))
}

pub async fn reset_browser(
    State(state): State<AppState>,
    Path((tenant_id, browser_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    let claims = authorize(&state, &headers, Some(tenant_id)).await?;

    // 409 — reject duplicate reset
    {
        let resets = state.pending_resets.read().await;
        if resets.contains_key(&browser_id) {
            return Err(AppError::conflict("reset already in progress"));
        }
    }

    let (snapshot, agent_id) = {
        let mut browsers = state.browsers.write().await;
        let browser = browsers
            .get_mut(&browser_id)
            .filter(|b| b.tenant_id == tenant_id)
            .ok_or_else(|| AppError::not_found("browser instance"))?;

        // 503 — reject when agent not connected
        let senders = state.agent_senders.read().await;
        let tx = senders
            .get(&browser.agent_id)
            .ok_or_else(|| AppError::service_unavailable("agent not connected"))?;

        let cmd = serde_json::to_string(&json!({
            "type": "browser.reset",
            "payload": { "browserInstanceId": browser_id }
        }))
        .unwrap_or_default();

        tx.try_send(cmd)
            .map_err(|_| AppError::service_unavailable("agent channel full"))?;

        browser.status = BrowserStatus::Restarting;
        browser.tabs = vec![];
        browser.active_tab_id = None;
        let snapshot = browser.clone();
        let agent_id = browser.agent_id;
        (snapshot, agent_id)
    };

    // Record pending reset
    {
        let mut resets = state.pending_resets.write().await;
        resets.insert(browser_id, Instant::now());
    }

    let _ = repo::create_audit_log(
        &state.pool,
        &repo::CreateAuditLog {
            id: Uuid::now_v7(),
            tenant_id,
            actor_type: "user",
            actor_id: &claims.sub,
            action: "browser.reset_requested",
            source: "web_ui",
            resource_type: None,
            resource_id: None,
            browser_instance_id: Some(browser_id),
            tab_id: None,
            metadata: None,
        },
    )
    .await;

    let _ = agent_id; // used above via senders lookup
    Ok(Json(json!({ "data": snapshot })))
}

pub async fn delete_browser(
    State(state): State<AppState>,
    Path((tenant_id, browser_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<StatusCode, AppError> {
    authorize(&state, &headers, Some(tenant_id)).await?;
    let removed = {
        let mut browsers = state.browsers.write().await;
        browsers
            .remove(&browser_id)
            .filter(|b| b.tenant_id == tenant_id)
            .is_some()
    };
    if removed {
        state.browser_preview.write().await.remove(&browser_id);
        state.browser_last_frame.write().await.remove(&browser_id);
        state.browser_events.write().await.remove(&browser_id);
        let _ = repo::delete_browser_instance(&state.pool, tenant_id, browser_id).await;
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::not_found("browser instance"))
    }
}
