use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Instant;
use uuid::Uuid;

use jbrowser_shared::models::{BrowserStatus, StealthLevel};

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

// ── Browser Config Endpoints ───────────────────────────────────────────────

pub async fn get_browser_config(
    State(state): State<AppState>,
    Path((tenant_id, browser_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    authorize(&state, &headers, Some(tenant_id)).await?;
    let browsers = state.browsers.read().await;
    let browser = browsers
        .get(&browser_id)
        .filter(|b| b.tenant_id == tenant_id)
        .ok_or_else(|| AppError::not_found("browser instance"))?;
    Ok(Json(json!({ "data": browser.config })))
}

#[derive(Debug, Deserialize)]
pub struct PatchBrowserConfigRequest {
    pub fingerprint: Option<PatchFingerprintConfig>,
    pub stealth: Option<StealthLevel>,
}

#[derive(Debug, Deserialize)]
pub struct PatchFingerprintConfig {
    pub user_agent: Option<String>,
    pub viewport_width: Option<u32>,
    pub viewport_height: Option<u32>,
    pub device_scale_factor: Option<f64>,
    pub timezone: Option<String>,
    pub locale: Option<String>,
}

pub async fn patch_browser_config(
    State(state): State<AppState>,
    Path((tenant_id, browser_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(req): Json<PatchBrowserConfigRequest>,
) -> Result<Json<Value>, AppError> {
    let claims = authorize(&state, &headers, Some(tenant_id)).await?;

    // Build the merged config
    let new_config = {
        let browsers = state.browsers.read().await;
        let browser = browsers
            .get(&browser_id)
            .filter(|b| b.tenant_id == tenant_id)
            .ok_or_else(|| AppError::not_found("browser instance"))?;

        let mut cfg = browser.config.clone();
        if let Some(fp) = &req.fingerprint {
            if fp.user_agent.is_some() {
                cfg.fingerprint.user_agent = fp.user_agent.clone();
            }
            if let Some(w) = fp.viewport_width {
                cfg.fingerprint.viewport_width = w;
            }
            if let Some(h) = fp.viewport_height {
                cfg.fingerprint.viewport_height = h;
            }
            if let Some(s) = fp.device_scale_factor {
                cfg.fingerprint.device_scale_factor = s;
            }
            if fp.timezone.is_some() {
                cfg.fingerprint.timezone = fp.timezone.clone();
            }
            if fp.locale.is_some() {
                cfg.fingerprint.locale = fp.locale.clone();
            }
        }
        if let Some(stealth) = &req.stealth {
            cfg.stealth = stealth.clone();
        }
        cfg
    };

    // Persist to DB
    let stealth_str = match &new_config.stealth {
        StealthLevel::None => "none",
        StealthLevel::Basic => "basic",
    };
    repo::update_browser_config(
        &state.pool,
        &repo::UpdateBrowserConfig {
            tenant_id,
            browser_id,
            user_agent: new_config.fingerprint.user_agent.as_deref(),
            viewport_width: new_config.fingerprint.viewport_width,
            viewport_height: new_config.fingerprint.viewport_height,
            device_scale_factor: new_config.fingerprint.device_scale_factor,
            timezone: new_config.fingerprint.timezone.as_deref(),
            locale: new_config.fingerprint.locale.as_deref(),
            stealth_level: stealth_str,
        },
    )
    .await
    .map_err(|e| AppError::internal(e.to_string()))?;

    // Update in-memory state
    {
        let mut browsers = state.browsers.write().await;
        if let Some(browser) = browsers.get_mut(&browser_id) {
            browser.viewport_width = new_config.fingerprint.viewport_width;
            browser.viewport_height = new_config.fingerprint.viewport_height;
            browser.config = new_config.clone();
        }
    }

    // Trigger browser.reset so agent restarts Chrome with new config
    {
        let browsers = state.browsers.read().await;
        if let Some(browser) = browsers.get(&browser_id) {
            let senders = state.agent_senders.read().await;
            if let Some(tx) = senders.get(&browser.agent_id) {
                let cmd = serde_json::to_string(&json!({
                    "type": "browser.reset",
                    "payload": {
                        "browserInstanceId": browser_id,
                        "config": new_config
                    }
                }))
                .unwrap_or_default();
                let _ = tx.try_send(cmd);
            }
        }
    }

    // Mark as restarting + record pending reset
    {
        let mut browsers = state.browsers.write().await;
        if let Some(browser) = browsers.get_mut(&browser_id) {
            browser.status = BrowserStatus::Restarting;
            browser.tabs = vec![];
            browser.active_tab_id = None;
        }
    }
    {
        let mut resets = state.pending_resets.write().await;
        resets.insert(browser_id, Instant::now());
    }

    // Audit log
    let _ = repo::create_audit_log(
        &state.pool,
        &repo::CreateAuditLog {
            id: Uuid::now_v7(),
            tenant_id,
            actor_type: "user",
            actor_id: &claims.sub,
            action: "browser.config_updated",
            source: "web_ui",
            resource_type: None,
            resource_id: None,
            browser_instance_id: Some(browser_id),
            tab_id: None,
            metadata: None,
        },
    )
    .await;

    Ok(Json(json!({ "data": new_config })))
}

/// Convenience endpoint: curated list of common User-Agents
pub async fn list_user_agents() -> Json<Value> {
    Json(json!({ "data": [
        { "label": "Chrome 125 / Windows", "value": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36" },
        { "label": "Chrome 125 / macOS", "value": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36" },
        { "label": "Chrome 125 / Linux", "value": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36" },
        { "label": "Safari 17 / macOS", "value": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15" },
        { "label": "Firefox 126 / Windows", "value": "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:126.0) Gecko/20100101 Firefox/126.0" },
    ] }))
}
