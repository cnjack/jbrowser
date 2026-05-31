use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::info;
use uuid::Uuid;

use jbrowser_shared::{
    constants::{DEFAULT_VIEWPORT_HEIGHT, DEFAULT_VIEWPORT_WIDTH},
    models::{AgentStatus, AgentSummary, BrowserConfig, BrowserInstance, BrowserStatus},
};

use crate::db::repo;
use crate::server::auth::crypto::{generate_token, hash_token};
use crate::server::auth::middleware::require_admin;
use crate::server::error::AppError;
use crate::server::state::AppState;

pub async fn list_agents(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    crate::server::auth::middleware::authorize(&state, &headers, Some(tenant_id)).await?;
    let agents = state.agents.read().await;
    let data: Vec<_> = agents
        .values()
        .filter(|agent| agent.tenant_id == tenant_id)
        .cloned()
        .collect();
    Ok(Json(json!({ "data": data })))
}

pub async fn get_agent(
    State(state): State<AppState>,
    Path((tenant_id, agent_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    crate::server::auth::middleware::authorize(&state, &headers, Some(tenant_id)).await?;
    let agents = state.agents.read().await;
    let agent = agents
        .get(&agent_id)
        .filter(|agent| agent.tenant_id == tenant_id)
        .cloned()
        .ok_or_else(|| AppError::not_found("agent"))?;
    Ok(Json(json!({ "data": agent })))
}

pub async fn delete_agent(
    State(state): State<AppState>,
    Path((tenant_id, agent_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<StatusCode, AppError> {
    require_admin(&state, &headers, tenant_id).await?;
    let agent = {
        let agents = state.agents.read().await;
        agents
            .get(&agent_id)
            .filter(|a| a.tenant_id == tenant_id)
            .cloned()
            .ok_or(AppError::not_found("agent"))?
    };
    state.agents.write().await.remove(&agent_id);
    let browser_id = agent.browser_instance_id;
    state.browsers.write().await.remove(&browser_id);
    state.agent_senders.write().await.remove(&agent_id);
    state.browser_preview.write().await.remove(&browser_id);
    state.browser_last_frame.write().await.remove(&browser_id);
    state.browser_events.write().await.remove(&browser_id);
    let _ = repo::delete_browser_instance(&state.pool, tenant_id, browser_id).await;
    let _ = repo::delete_agent(&state.pool, tenant_id, agent_id).await;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
pub struct AgentRegisterRequest {
    registration_token: String,
    name: Option<String>,
    browser_type: Option<String>,
    browser_version: Option<String>,
    agent_id: Option<String>,
    browser_instance_id: Option<String>,
}

pub async fn agent_register(
    State(state): State<AppState>,
    Json(req): Json<AgentRegisterRequest>,
) -> Result<Json<Value>, AppError> {
    let token_hash = hash_token(&req.registration_token);
    let registration = repo::get_token_by_hash(&state.pool, &token_hash)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?
        .ok_or_else(|| AppError::unauthorized("invalid registration token"))?;

    if registration.token_type != "agent_registration" {
        return Err(AppError::unauthorized("invalid registration token"));
    }

    let tenant_id = Uuid::parse_str(&registration.tenant_id).unwrap();
    let runtime_token = generate_token("jbr_agent");
    let browser_type = req.browser_type.unwrap_or_else(|| "chromium".to_string());
    let browser_version = req.browser_version.unwrap_or_else(|| "unknown".to_string());

    // Re-registration: agent already has a stable identity
    if let (Some(existing_agent_id_str), Some(existing_browser_id_str)) =
        (req.agent_id.as_deref(), req.browser_instance_id.as_deref())
    {
        if let (Ok(existing_agent_id), Ok(existing_browser_id)) = (
            Uuid::parse_str(existing_agent_id_str),
            Uuid::parse_str(existing_browser_id_str),
        ) {
            if let Ok(Some(agent_row)) =
                repo::get_agent(&state.pool, tenant_id, existing_agent_id).await
            {
                let agent_name = agent_row
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("agent-{existing_agent_id}"));

                let _ = repo::update_agent_runtime_token(
                    &state.pool,
                    existing_agent_id,
                    &hash_token(&runtime_token),
                )
                .await;
                let _ = repo::replace_agent_runtime_token(
                    &state.pool,
                    existing_agent_id,
                    tenant_id,
                    &hash_token(&runtime_token),
                    &runtime_token.chars().take(12).collect::<String>(),
                )
                .await;

                let agent = AgentSummary {
                    id: existing_agent_id,
                    tenant_id,
                    browser_instance_id: existing_browser_id,
                    name: agent_name.clone(),
                    status: AgentStatus::Offline,
                    browser_type: browser_type.clone(),
                    browser_version: browser_version.clone(),
                    last_heartbeat_at: None,
                };
                let browser = BrowserInstance {
                    id: existing_browser_id,
                    tenant_id,
                    agent_id: existing_agent_id,
                    name: format!("{browser_type}-{existing_browser_id}"),
                    status: BrowserStatus::Offline,
                    browser_type: browser_type.clone(),
                    browser_version: browser_version.clone(),
                    active_tab_id: None,
                    tabs: Vec::new(),
                    proxy_enabled: false,
                    viewport_width: DEFAULT_VIEWPORT_WIDTH,
                    viewport_height: DEFAULT_VIEWPORT_HEIGHT,
                    viewer_count: 0,
                    agent_name: agent_name.clone(),
                    agent_status: AgentStatus::Offline,
                    last_heartbeat_at: None,
                    config: BrowserConfig::default(),
                };
                state.agents.write().await.insert(existing_agent_id, agent);
                state
                    .browsers
                    .write()
                    .await
                    .insert(existing_browser_id, browser);

                info!(%existing_agent_id, "agent re-registered with stable identity");
                return Ok(Json(json!({
                    "agent_id": existing_agent_id,
                    "browser_instance_id": existing_browser_id,
                    "agent_runtime_token": runtime_token
                })));
            }
        }
    }

    // First-time registration: create new identity
    let agent_id = Uuid::now_v7();
    let browser_id = Uuid::now_v7();
    let agent_name = req.name.unwrap_or_else(|| format!("agent-{agent_id}"));

    let agent = AgentSummary {
        id: agent_id,
        tenant_id,
        browser_instance_id: browser_id,
        name: agent_name.clone(),
        status: AgentStatus::Offline,
        browser_type: browser_type.clone(),
        browser_version: browser_version.clone(),
        last_heartbeat_at: None,
    };
    let browser = BrowserInstance {
        id: browser_id,
        tenant_id,
        agent_id,
        name: format!("{browser_type}-{browser_id}"),
        status: BrowserStatus::Offline,
        browser_type: browser_type.clone(),
        browser_version: browser_version.clone(),
        active_tab_id: None,
        tabs: Vec::new(),
        proxy_enabled: false,
        viewport_width: DEFAULT_VIEWPORT_WIDTH,
        viewport_height: DEFAULT_VIEWPORT_HEIGHT,
        viewer_count: 0,
        agent_name: agent_name.clone(),
        agent_status: AgentStatus::Offline,
        last_heartbeat_at: None,
        config: BrowserConfig::default(),
    };

    let _ = repo::create_agent(
        &state.pool,
        agent_id,
        tenant_id,
        browser_id,
        &agent_name,
        &hash_token(&runtime_token),
    )
    .await;
    let _ = repo::create_browser_instance(
        &state.pool,
        browser_id,
        tenant_id,
        agent_id,
        &browser_type,
        &browser_version,
    )
    .await;
    let runtime_record_id = Uuid::now_v7();
    let _ = repo::create_token(
        &state.pool,
        &repo::CreateToken {
            id: runtime_record_id,
            tenant_id,
            token_type: "agent_runtime",
            name: Some(&format!("runtime-{agent_id}")),
            token_hash: &hash_token(&runtime_token),
            token_prefix: &runtime_token.chars().take(12).collect::<String>(),
            created_by: None,
        },
    )
    .await;
    let _ = repo::create_audit_log(
        &state.pool,
        &repo::CreateAuditLog {
            id: Uuid::now_v7(),
            tenant_id,
            actor_type: "agent",
            actor_id: &agent_id.to_string(),
            action: "agent.registered",
            source: "agent",
            resource_type: None,
            resource_id: None,
            browser_instance_id: Some(browser_id),
            tab_id: None,
            metadata: None,
        },
    )
    .await;

    state.agents.write().await.insert(agent_id, agent);
    state.browsers.write().await.insert(browser_id, browser);

    Ok(Json(json!({
        "agent_id": agent_id,
        "browser_instance_id": browser_id,
        "agent_runtime_token": runtime_token
    })))
}
