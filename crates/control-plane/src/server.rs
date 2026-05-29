use std::{collections::HashMap, env, sync::Arc, time::Duration};

use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post, delete},
    Json, Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use futures_util::{SinkExt, StreamExt};
use jbrowser_shared::{
    constants::{DEFAULT_VIEWPORT_HEIGHT, DEFAULT_VIEWPORT_WIDTH},
    models::{AgentStatus, AgentSummary, BrowserInstance, BrowserStatus, BrowserTab},
    protocol::{decode_video_frame, ClientMessage, ServerMessage},
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::sync::{broadcast, mpsc, RwLock};
use tower_http::{
    cors::CorsLayer,
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
use tracing::info;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub jwt_secret: String,
    pub database_url: String,
    pub public_base_url: String,
    pub demo_email: String,
    pub demo_password: String,
    /// If set, a seed agent registration token is always available in the store.
    pub seed_agent_token: Option<String>,
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            host: env::var("JBROWSER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("PORT")
                .or_else(|_| env::var("JBROWSER_PORT"))
                .unwrap_or_else(|_| "8080".to_string())
                .parse()?,
            jwt_secret: required_env("JWT_SECRET")?,
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "mysql://root:jbrowser@localhost:3306/jbrowser".to_string()),
            public_base_url: env::var("PUBLIC_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),
            demo_email: env::var("DEMO_EMAIL").unwrap_or_else(|_| "admin@example.com".to_string()),
            demo_password: env::var("DEMO_PASSWORD").unwrap_or_else(|_| "jbrowser".to_string()),
            seed_agent_token: env::var("SEED_AGENT_TOKEN").ok(),
        })
    }
}

fn required_env(name: &str) -> anyhow::Result<String> {
    env::var(name).map_err(|_| anyhow::anyhow!("missing required env var: {name}"))
}

#[derive(Clone)]
pub struct AppState {
    config: AppConfig,
    store: Arc<RwLock<Store>>,
    /// Global broadcast of raw video bytes (legacy, replaced by per-browser channels)
    preview_tx: broadcast::Sender<Vec<u8>>,
    /// Per-agent command senders: agent_id → mpsc::Sender<String (JSON)>
    agent_senders: Arc<RwLock<HashMap<Uuid, mpsc::Sender<String>>>>,
    /// Per-browser video broadcast: browser_id → broadcast::Sender<Vec<u8>>
    browser_preview: Arc<RwLock<HashMap<Uuid, broadcast::Sender<Vec<u8>>>>>,
    /// Per-browser event broadcast: browser_id → broadcast::Sender<String (JSON)>
    browser_events: Arc<RwLock<HashMap<Uuid, broadcast::Sender<String>>>>,
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        let store = Store::seed(&config);
        let (preview_tx, _) = broadcast::channel(64);
        Self {
            config,
            store: Arc::new(RwLock::new(store)),
            preview_tx,
            agent_senders: Arc::new(RwLock::new(HashMap::new())),
            browser_preview: Arc::new(RwLock::new(HashMap::new())),
            browser_events: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[derive(Debug)]
struct Store {
    tenants: HashMap<Uuid, Tenant>,
    users: HashMap<Uuid, User>,
    browsers: HashMap<Uuid, BrowserInstance>,
    agents: HashMap<Uuid, AgentSummary>,
    tokens: HashMap<Uuid, TokenRecord>,
    audit_logs: Vec<AuditLog>,
}

impl Store {
    fn seed(config: &AppConfig) -> Self {
        let tenant_id = Uuid::now_v7();
        let user_id = Uuid::now_v7();
        let agent_id = Uuid::now_v7();
        let browser_id = Uuid::now_v7();
        let password_hash = hash_password(&config.demo_password).expect("demo password hash");
        let tab = BrowserTab {
            id: "demo-tab".to_string(),
            title: "JBrowser Demo".to_string(),
            url: "about:blank".to_string(),
            active: true,
        };
        let agent = AgentSummary {
            id: agent_id,
            tenant_id,
            browser_instance_id: browser_id,
            name: "demo-agent".to_string(),
            status: AgentStatus::Offline,
            browser_type: "chromium".to_string(),
            browser_version: "demo".to_string(),
            last_heartbeat_at: None,
        };
        let browser = BrowserInstance {
            id: browser_id,
            tenant_id,
            agent_id,
            name: "demo-browser".to_string(),
            status: BrowserStatus::Offline,
            browser_type: "chromium".to_string(),
            browser_version: "demo".to_string(),
            active_tab_id: Some(tab.id.clone()),
            tabs: vec![tab],
            proxy_enabled: false,
            viewport_width: DEFAULT_VIEWPORT_WIDTH,
            viewport_height: DEFAULT_VIEWPORT_HEIGHT,
            viewer_count: 0,
            agent_name: agent.name.clone(),
            agent_status: agent.status.clone(),
            last_heartbeat_at: None,
        };

        let mut tokens: HashMap<Uuid, TokenRecord> = HashMap::new();

        // If a seed agent token is configured, register it so the agent can connect
        // even after control-plane restarts (no manual re-registration needed).
        if let Some(raw_token) = &config.seed_agent_token {
            let token_id = Uuid::now_v7();
            tokens.insert(token_id, TokenRecord {
                id: token_id,
                tenant_id,
                token_type: TokenType::AgentRegistration,
                name: Some("seed-agent-token".to_string()),
                token_hash: hash_token(raw_token),
                token_prefix: raw_token.chars().take(12).collect(),
                created_by: Some(user_id.to_string()),
                revoked_at: None,
                created_at: Utc::now(),
            });
        }

        Self {
            tenants: HashMap::from([(
                tenant_id,
                Tenant {
                    id: tenant_id,
                    name: "Default Tenant".to_string(),
                    slug: "default".to_string(),
                },
            )]),
            users: HashMap::from([(
                user_id,
                User {
                    id: user_id,
                    email: config.demo_email.clone(),
                    display_name: "Demo Admin".to_string(),
                    password_hash,
                    tenants: vec![TenantClaim {
                        id: tenant_id,
                        role: "admin".to_string(),
                    }],
                },
            )]),
            browsers: HashMap::from([(browser_id, browser)]),
            agents: HashMap::from([(agent_id, agent)]),
            tokens,
            audit_logs: Vec::new(),
        }
    }
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/metrics", get(metrics))
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/auth/me", get(me))
        .route("/api/v1/agents/register", post(agent_register))
        .route("/api/v1/agents/connect", get(ws_agent))
        .route(
            "/api/v1/tenants/:tenant_id/browser-instances",
            get(list_browsers),
        )
        .route(
            "/api/v1/tenants/:tenant_id/browser-instances/:browser_id",
            get(get_browser).delete(delete_browser),
        )
        .route(
            "/api/v1/tenants/:tenant_id/browser-instances/:browser_id/reset",
            post(reset_browser),
        )
        .route("/api/v1/tenants/:tenant_id/agents", get(list_agents))
        .route(
            "/api/v1/tenants/:tenant_id/agents/:agent_id",
            get(get_agent).delete(delete_agent),
        )
        .route(
            "/api/v1/tenants/:tenant_id/tokens/cdp",
            get(list_cdp_tokens).post(create_cdp_token),
        )
        .route(
            "/api/v1/tenants/:tenant_id/tokens/cdp/:token_id/revoke",
            post(revoke_token),
        )
        .route(
            "/api/v1/tenants/:tenant_id/tokens/cdp/:token_id/rotate",
            post(rotate_token),
        )
        .route(
            "/api/v1/tenants/:tenant_id/agent-registration-tokens",
            post(create_agent_registration_token),
        )
        .route(
            "/api/v1/tenants/:tenant_id/agent-registration-tokens/:token_id/revoke",
            post(revoke_token),
        )
        .route(
            "/api/v1/tenants/:tenant_id/audit-logs",
            get(list_audit_logs),
        )
        .route("/ws/control", get(ws_control))
        .route(
            "/cdp/tenants/:tenant_id/browser-instances/:browser_id/json/version",
            get(cdp_json_version),
        )
        .route(
            "/cdp/tenants/:tenant_id/browser-instances/:browser_id/json/list",
            get(cdp_json_list),
        )
        .route(
            "/cdp/tenants/:tenant_id/browser-instances/:browser_id/devtools/browser/:target_id",
            get(cdp_ws_placeholder),
        )
        .route(
            "/cdp/tenants/:tenant_id/browser-instances/:browser_id/devtools/page/:target_id",
            get(cdp_ws_placeholder),
        )
        .fallback_service(
            ServeDir::new("frontend/dist")
                .fallback(ServeFile::new("frontend/dist/index.html")),
        )
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

async fn ready(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "checks": {
            "database_url_configured": !state.config.database_url.is_empty(),
            "state": "in_memory_mvp"
        }
    }))
}

async fn metrics(State(state): State<AppState>) -> String {
    let store = state.store.read().await;
    let online = store
        .browsers
        .values()
        .filter(|browser| browser.status == BrowserStatus::Online)
        .count();
    format!(
        "# HELP browser_instances_online Online browser instances\n# TYPE browser_instances_online gauge\nbrowser_instances_online {}\n",
        online
    )
}

#[derive(Debug, Deserialize)]
struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Debug, Serialize)]
struct LoginResponse {
    access_token: String,
    token_type: &'static str,
    user: UserResponse,
    tenants: Vec<Tenant>,
}

async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AppError> {
    let mut store = state.store.write().await;
    let user = store
        .users
        .values()
        .find(|user| user.email == req.email)
        .cloned()
        .ok_or(AppError::unauthorized("invalid credentials"))?;

    verify_password(&req.password, &user.password_hash)
        .map_err(|_| AppError::unauthorized("invalid credentials"))?;

    let access_token = issue_jwt(&state.config.jwt_secret, &user)?;
    let tenants = user
        .tenants
        .iter()
        .filter_map(|claim| store.tenants.get(&claim.id).cloned())
        .collect::<Vec<_>>();
    store.audit_logs.push(AuditLog::new(
        user.tenants[0].id,
        "user",
        user.id.to_string(),
        "user.login",
        "web_ui",
        None,
    ));

    Ok(Json(LoginResponse {
        access_token,
        token_type: "Bearer",
        user: UserResponse::from(user),
        tenants,
    }))
}

async fn me(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<Value>, AppError> {
    let claims = authorize(&state, &headers, None).await?;
    Ok(Json(json!({
        "user": {
            "id": claims.sub,
            "email": claims.email,
            "tenants": claims.tenants
        }
    })))
}

async fn list_browsers(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    authorize(&state, &headers, Some(tenant_id)).await?;
    let store = state.store.read().await;
    let data = store
        .browsers
        .values()
        .filter(|browser| browser.tenant_id == tenant_id)
        .cloned()
        .collect::<Vec<_>>();
    Ok(Json(json!({ "data": data })))
}

async fn get_browser(
    State(state): State<AppState>,
    Path((tenant_id, browser_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    authorize(&state, &headers, Some(tenant_id)).await?;
    let store = state.store.read().await;
    let browser = store
        .browsers
        .get(&browser_id)
        .filter(|browser| browser.tenant_id == tenant_id)
        .cloned()
        .ok_or_else(|| AppError::not_found("browser instance"))?;
    Ok(Json(json!({ "data": browser })))
}

async fn reset_browser(
    State(state): State<AppState>,
    Path((tenant_id, browser_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    let claims = authorize(&state, &headers, Some(tenant_id)).await?;
    let (snapshot, agent_id) = {
        let mut store = state.store.write().await;
        let browser = store
            .browsers
            .get_mut(&browser_id)
            .filter(|browser| browser.tenant_id == tenant_id)
            .ok_or_else(|| AppError::not_found("browser instance"))?;
        browser.status = BrowserStatus::Restarting;
        browser.tabs = vec![BrowserTab {
            id: "blank".to_string(),
            title: "New Tab".to_string(),
            url: "about:blank".to_string(),
            active: true,
        }];
        browser.active_tab_id = Some("blank".to_string());
        let snapshot = browser.clone();
        let agent_id = browser.agent_id;
        store.audit_logs.push(AuditLog::new(
            tenant_id,
            "user",
            claims.sub,
            "browser.reset_requested",
            "web_ui",
            Some(browser_id),
        ));
        (snapshot, agent_id)
    };

    // Forward reset to agent
    {
        let senders = state.agent_senders.read().await;
        if let Some(tx) = senders.get(&agent_id) {
            let cmd = serde_json::to_string(&serde_json::json!({
                "type": "browser.reset",
                "payload": { "browserInstanceId": browser_id }
            }))
            .unwrap_or_default();
            let _ = tx.try_send(cmd);
        }
    }

    Ok(Json(json!({ "data": snapshot })))
}

async fn list_agents(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    authorize(&state, &headers, Some(tenant_id)).await?;
    let store = state.store.read().await;
    let data = store
        .agents
        .values()
        .filter(|agent| agent.tenant_id == tenant_id)
        .cloned()
        .collect::<Vec<_>>();
    Ok(Json(json!({ "data": data })))
}

async fn get_agent(
    State(state): State<AppState>,
    Path((tenant_id, agent_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    authorize(&state, &headers, Some(tenant_id)).await?;
    let store = state.store.read().await;
    let agent = store
        .agents
        .get(&agent_id)
        .filter(|agent| agent.tenant_id == tenant_id)
        .cloned()
        .ok_or_else(|| AppError::not_found("agent"))?;
    Ok(Json(json!({ "data": agent })))
}

async fn delete_agent(
    State(state): State<AppState>,
    Path((tenant_id, agent_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<StatusCode, AppError> {
    authorize(&state, &headers, Some(tenant_id)).await?;
    let mut store = state.store.write().await;
    let removed = store
        .agents
        .remove(&agent_id)
        .filter(|a| a.tenant_id == tenant_id)
        .is_some();
    if removed {
        // Also remove the associated browser instance
        store.browsers.retain(|_, b| b.agent_id != agent_id);
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::not_found("agent"))
    }
}

async fn delete_browser(
    State(state): State<AppState>,
    Path((tenant_id, browser_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<StatusCode, AppError> {
    authorize(&state, &headers, Some(tenant_id)).await?;
    let mut store = state.store.write().await;
    let removed = store
        .browsers
        .remove(&browser_id)
        .filter(|b| b.tenant_id == tenant_id)
        .is_some();
    if removed {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::not_found("browser instance"))
    }
}

async fn list_cdp_tokens(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    authorize(&state, &headers, Some(tenant_id)).await?;
    let store = state.store.read().await;
    let data = store
        .tokens
        .values()
        .filter(|token| {
            token.tenant_id == tenant_id && token.token_type == TokenType::TenantCdpAccess
        })
        .map(TokenResponse::from)
        .collect::<Vec<_>>();
    Ok(Json(json!({ "data": data })))
}

#[derive(Debug, Deserialize)]
struct CreateTokenRequest {
    name: Option<String>,
}

async fn create_cdp_token(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
    headers: HeaderMap,
    body: Option<Json<CreateTokenRequest>>,
) -> Result<Json<Value>, AppError> {
    let name = body.and_then(|Json(req)| req.name);
    create_token(
        state,
        tenant_id,
        headers,
        TokenType::TenantCdpAccess,
        name,
    )
    .await
}

async fn create_agent_registration_token(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
    headers: HeaderMap,
    body: Option<Json<CreateTokenRequest>>,
) -> Result<Json<Value>, AppError> {
    let name = body.and_then(|Json(req)| req.name);
    create_token(
        state,
        tenant_id,
        headers,
        TokenType::AgentRegistration,
        name,
    )
    .await
}

async fn create_token(
    state: AppState,
    tenant_id: Uuid,
    headers: HeaderMap,
    token_type: TokenType,
    name: Option<String>,
) -> Result<Json<Value>, AppError> {
    let claims = authorize(&state, &headers, Some(tenant_id)).await?;
    let raw = generate_token(match token_type {
        TokenType::AgentRegistration => "jbr_reg",
        TokenType::AgentRuntime => "jbr_agent",
        TokenType::TenantCdpAccess => "jbr_cdp",
    });
    let record = TokenRecord {
        id: Uuid::now_v7(),
        tenant_id,
        token_type,
        name,
        token_hash: hash_token(&raw),
        token_prefix: raw.chars().take(12).collect(),
        created_by: Some(claims.sub.clone()),
        revoked_at: None,
        created_at: Utc::now(),
    };
    let response = json!({
        "data": TokenResponse::from(&record),
        "token": raw
    });
    let mut store = state.store.write().await;
    store.audit_logs.push(AuditLog::new(
        tenant_id,
        "user",
        claims.sub,
        "token.created",
        "web_ui",
        None,
    ));
    store.tokens.insert(record.id, record);
    Ok(Json(response))
}

async fn revoke_token(
    State(state): State<AppState>,
    Path((tenant_id, token_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    let claims = authorize(&state, &headers, Some(tenant_id)).await?;
    let mut store = state.store.write().await;
    let token = store
        .tokens
        .get_mut(&token_id)
        .filter(|token| token.tenant_id == tenant_id)
        .ok_or_else(|| AppError::not_found("token"))?;
    token.revoked_at = Some(Utc::now());
    let response = TokenResponse::from(&*token);
    store.audit_logs.push(AuditLog::new(
        tenant_id,
        "user",
        claims.sub,
        "token.revoked",
        "web_ui",
        None,
    ));
    Ok(Json(json!({ "data": response })))
}

async fn rotate_token(
    State(state): State<AppState>,
    Path((tenant_id, token_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    let claims = authorize(&state, &headers, Some(tenant_id)).await?;
    let mut store = state.store.write().await;
    let token = store
        .tokens
        .get_mut(&token_id)
        .filter(|token| {
            token.tenant_id == tenant_id && token.token_type == TokenType::TenantCdpAccess
        })
        .ok_or_else(|| AppError::not_found("CDP token"))?;
    let raw = generate_token("jbr_cdp");
    token.token_hash = hash_token(&raw);
    token.token_prefix = raw.chars().take(12).collect();
    token.revoked_at = None;
    let response = TokenResponse::from(&*token);
    store.audit_logs.push(AuditLog::new(
        tenant_id,
        "user",
        claims.sub,
        "token.created",
        "web_ui",
        None,
    ));
    Ok(Json(json!({ "data": response, "token": raw })))
}

async fn list_audit_logs(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    authorize(&state, &headers, Some(tenant_id)).await?;
    let store = state.store.read().await;
    let data = store
        .audit_logs
        .iter()
        .filter(|log| log.tenant_id == tenant_id)
        .cloned()
        .collect::<Vec<_>>();
    Ok(Json(json!({ "data": data })))
}

#[derive(Debug, Deserialize)]
struct AgentRegisterRequest {
    registration_token: String,
    name: Option<String>,
    browser_type: Option<String>,
    browser_version: Option<String>,
}

async fn agent_register(
    State(state): State<AppState>,
    Json(req): Json<AgentRegisterRequest>,
) -> Result<Json<Value>, AppError> {
    let token_hash = hash_token(&req.registration_token);
    let mut store = state.store.write().await;
    let registration = store
        .tokens
        .values()
        .find(|token| {
            token.token_hash == token_hash
                && token.token_type == TokenType::AgentRegistration
                && token.revoked_at.is_none()
        })
        .cloned()
        .ok_or_else(|| AppError::unauthorized("invalid registration token"))?;

    let agent_id = Uuid::now_v7();
    let browser_id = Uuid::now_v7();
    let runtime_token = generate_token("jbr_agent");
    let browser_type = req.browser_type.unwrap_or_else(|| "chromium".to_string());
    let browser_version = req.browser_version.unwrap_or_else(|| "unknown".to_string());
    let agent = AgentSummary {
        id: agent_id,
        tenant_id: registration.tenant_id,
        browser_instance_id: browser_id,
        name: req.name.unwrap_or_else(|| format!("agent-{agent_id}")),
        status: AgentStatus::Offline,
        browser_type: browser_type.clone(),
        browser_version: browser_version.clone(),
        last_heartbeat_at: None,
    };
    let browser = BrowserInstance {
        id: browser_id,
        tenant_id: registration.tenant_id,
        agent_id,
        name: format!("{browser_type}-{browser_id}"),
        status: BrowserStatus::Offline,
        browser_type,
        browser_version,
        active_tab_id: None,
        tabs: Vec::new(),
        proxy_enabled: false,
        viewport_width: DEFAULT_VIEWPORT_WIDTH,
        viewport_height: DEFAULT_VIEWPORT_HEIGHT,
        viewer_count: 0,
        agent_name: agent.name.clone(),
        agent_status: AgentStatus::Offline,
        last_heartbeat_at: None,
    };
    store.audit_logs.push(AuditLog::new(
        registration.tenant_id,
        "agent",
        agent_id.to_string(),
        "agent.registered",
        "agent",
        Some(browser_id),
    ));
    store.agents.insert(agent_id, agent);
    store.browsers.insert(browser_id, browser);
    let runtime_record = TokenRecord {
        id: Uuid::now_v7(),
        tenant_id: registration.tenant_id,
        token_type: TokenType::AgentRuntime,
        name: Some(format!("runtime-{agent_id}")),
        token_hash: hash_token(&runtime_token),
        token_prefix: runtime_token.chars().take(12).collect(),
        created_by: None,
        revoked_at: None,
        created_at: Utc::now(),
    };
    store.tokens.insert(runtime_record.id, runtime_record);

    Ok(Json(json!({
        "agent_id": agent_id,
        "browser_instance_id": browser_id,
        "agent_runtime_token": runtime_token
    })))
}

async fn ws_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<Response, AppError> {
    let token =
        bearer_token(&headers).ok_or_else(|| AppError::unauthorized("missing agent token"))?;
    let token_hash = hash_token(token);
    let store = state.store.read().await;
    let token = store
        .tokens
        .values()
        .find(|record| {
            record.token_hash == token_hash
                && record.token_type == TokenType::AgentRuntime
                && record.revoked_at.is_none()
        })
        .cloned()
        .ok_or_else(|| AppError::unauthorized("invalid agent token"))?;
    // Derive agent_id from token name "runtime-{agent_id}"
    let agent_id_from_token = token
        .name
        .as_deref()
        .and_then(|n| n.strip_prefix("runtime-"))
        .and_then(|s| Uuid::parse_str(s).ok());
    drop(store);
    Ok(ws
        .on_upgrade(move |socket| {
            handle_agent_socket(state, token.tenant_id, agent_id_from_token, socket)
        })
        .into_response())
}

async fn handle_agent_socket(
    state: AppState,
    tenant_id: Uuid,
    agent_id_hint: Option<Uuid>,
    mut socket: WebSocket,
) {
    info!(%tenant_id, "agent websocket connected");

    // Look up the exact agent using the hint from the runtime token name.
    // Fall back to any agent for the tenant if the hint is unavailable.
    let (agent_id, browser_id) = {
        let store = state.store.read().await;
        let agent = agent_id_hint
            .and_then(|id| store.agents.get(&id).cloned())
            .or_else(|| {
                store
                    .agents
                    .values()
                    .find(|a| a.tenant_id == tenant_id)
                    .cloned()
            });
        match agent {
            Some(a) => (a.id, a.browser_instance_id),
            None => {
                info!(%tenant_id, "no agent found for tenant, closing socket");
                return;
            }
        }
    };

    // Create per-browser video channel
    let (bcast_tx, _) = broadcast::channel::<Vec<u8>>(128);
    state.browser_preview.write().await.insert(browser_id, bcast_tx.clone());

    // Create per-browser event channel (tab.list etc.)
    let (event_tx, _) = broadcast::channel::<String>(64);
    state.browser_events.write().await.insert(browser_id, event_tx.clone());

    // Create per-agent command channel
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<String>(64);
    state.agent_senders.write().await.insert(agent_id, cmd_tx);

    let (mut ws_tx, mut ws_rx) = socket.split();

    loop {
        tokio::select! {
            // Messages from agent → process heartbeats / video frames
            msg = ws_rx.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(value) = serde_json::from_str::<Value>(&text) {
                            // Broadcast tab.list events to subscribed frontend clients
                            if value.get("type").and_then(Value::as_str) == Some("tab.list") {
                                let _ = event_tx.send(text.clone());
                            }
                            handle_agent_text(&state, agent_id, browser_id, value).await;
                        }
                    }
                    Some(Ok(Message::Binary(bytes))) => {
                        if decode_video_frame(bytes.clone().into()).is_ok() {
                            let _ = bcast_tx.send(bytes.clone());
                            let _ = state.preview_tx.send(bytes);
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
            // Commands from control plane → forward to agent
            Some(cmd) = cmd_rx.recv() => {
                if ws_tx.send(Message::Text(cmd)).await.is_err() {
                    break;
                }
            }
        }
    }

    // Cleanup on disconnect
    state.agent_senders.write().await.remove(&agent_id);
    state.browser_preview.write().await.remove(&browser_id);
    state.browser_events.write().await.remove(&browser_id);
    let mut store = state.store.write().await;
    if let Some(a) = store.agents.get_mut(&agent_id) {
        a.status = AgentStatus::Offline;
    }
    if let Some(b) = store.browsers.get_mut(&browser_id) {
        b.status = BrowserStatus::Offline;
        b.agent_status = AgentStatus::Offline;
    }
    info!(%tenant_id, %agent_id, "agent websocket disconnected");
}

async fn handle_agent_text(state: &AppState, agent_id: Uuid, browser_id: Uuid, value: Value) {
    let now = Utc::now().to_rfc3339();
    let mut store = state.store.write().await;
    if let Some(agent) = store.agents.get_mut(&agent_id) {
        agent.status = AgentStatus::Online;
        agent.last_heartbeat_at = Some(now.clone());
    }
    if let Some(browser) = store.browsers.get_mut(&browser_id) {
        browser.status = BrowserStatus::Online;
        browser.agent_status = AgentStatus::Online;
        browser.last_heartbeat_at = Some(now.clone());
        if value.get("type").and_then(Value::as_str) == Some("tab.list") {
            if let Some(tabs) = value.get("payload").and_then(|payload| payload.get("tabs")) {
                if let Ok(parsed) = serde_json::from_value::<Vec<BrowserTab>>(tabs.clone()) {
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

async fn ws_control(State(state): State<AppState>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |socket| handle_control_socket(state, socket))
        .into_response()
}

async fn handle_control_socket(state: AppState, socket: WebSocket) {
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

    // Per-browser video subscription, updated when client sends browser.subscribe
    let mut current_browser_id: Option<Uuid> = None;
    let mut preview_rx: Option<broadcast::Receiver<Vec<u8>>> = None;
    let mut events_rx: Option<broadcast::Receiver<String>> = None;

    loop {
        // Build future for video segment (only if subscribed)
        let video_fut = async {
            match preview_rx.as_mut() {
                Some(rx) => match rx.recv().await {
                    Ok(bytes) => Some(bytes),
                    Err(_) => None, // lagged or channel closed → just skip
                },
                None => std::future::pending().await,
            }
        };

        // Build future for browser events (tab.list etc.)
        let event_fut = async {
            match events_rx.as_mut() {
                Some(rx) => match rx.recv().await {
                    Ok(msg) => Some(msg),
                    Err(_) => None,
                },
                None => std::future::pending().await,
            }
        };

        tokio::select! {
            Some(Ok(message)) = receiver.next() => {
                if let Message::Text(text) = message {
                    // Check if this is a browser.subscribe to update preview channel
                    if let Ok(parsed) = serde_json::from_str::<ClientMessage>(&text) {
                        if parsed.kind == "browser.subscribe" {
                            if let Some(bid) = parsed.payload.get("browserInstanceId")
                                .and_then(Value::as_str)
                                .and_then(|id| Uuid::parse_str(id).ok())
                            {
                                current_browser_id = Some(bid);
                                // Subscribe to per-browser preview channel if available
                                let map = state.browser_preview.read().await;
                                preview_rx = map.get(&bid).map(|tx| tx.subscribe());
                                // Subscribe to per-browser event channel
                                let ev_map = state.browser_events.read().await;
                                events_rx = ev_map.get(&bid).map(|tx| tx.subscribe());
                            }
                        }
                    }
                    handle_control_text(&state, &claims, &mut sender, &text, current_browser_id).await;
                }
            }
            Some(event_json) = event_fut => {
                // Forward tab.list / browser events to subscribed frontend client
                if sender.send(Message::Text(event_json)).await.is_err() {
                    return;
                }
            }
            Some(bytes) = video_fut => {
                if sender.send(Message::Binary(bytes)).await.is_err() {
                    return;
                }
                // Refresh subscription if the channel was recreated (agent reconnected)
                if let Some(bid) = current_browser_id {
                    if preview_rx.as_mut().map(|rx| rx.len() == 0).unwrap_or(false) {
                        // still valid, do nothing
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
            let store = state.store.read().await;
            if let Some(browser) = store.browsers.get(&browser_id).filter(|browser| {
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
        "input.event" | "tab.command" | "browser.reset" | "navigate.url" => {
            // Resolve target browser
            let browser_id = message
                .payload
                .get("browserInstanceId")
                .and_then(Value::as_str)
                .and_then(|id| Uuid::parse_str(id).ok())
                .or(current_browser_id);

            let mut store = state.store.write().await;
            if let Some(tenant) = claims.tenants.first() {
                store.audit_logs.push(AuditLog::new(
                    tenant.id,
                    "user",
                    claims.sub.clone(),
                    match message.kind.as_str() {
                        "input.event" => "input.dispatched",
                        "tab.command" => "tab.command_requested",
                        "navigate.url" => "navigate.requested",
                        _ => "browser.reset_requested",
                    },
                    "web_ui",
                    None,
                ));
            }

            // Forward command to agent if we know which browser/agent
            if let Some(bid) = browser_id {
                if let Some(browser) = store.browsers.get(&bid) {
                    let agent_id = browser.agent_id;
                    drop(store);
                    let senders = state.agent_senders.read().await;
                    if let Some(tx) = senders.get(&agent_id) {
                        let cmd = serde_json::to_string(&message).unwrap_or_default();
                        let _ = tx.try_send(cmd);
                    }
                } else {
                    drop(store);
                }
            } else {
                drop(store);
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

async fn send_json(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    message: ServerMessage,
) -> Result<(), axum::Error> {
    sender
        .send(Message::Text(
            serde_json::to_string(&message).expect("server message serialization"),
        ))
        .await
}

#[derive(Debug, Deserialize)]
struct CdpQuery {
    token: String,
}

async fn cdp_json_version(
    State(state): State<AppState>,
    Path((tenant_id, browser_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<CdpQuery>,
) -> Result<Json<Value>, AppError> {
    validate_cdp_token(&state, tenant_id, &query.token).await?;
    let store = state.store.read().await;
    let browser = store
        .browsers
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

async fn cdp_json_list(
    State(state): State<AppState>,
    Path((tenant_id, browser_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<CdpQuery>,
) -> Result<Json<Value>, AppError> {
    validate_cdp_token(&state, tenant_id, &query.token).await?;
    let store = state.store.read().await;
    let browser = store
        .browsers
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

async fn cdp_ws_placeholder(ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(|mut socket| async move {
        let _ = socket
            .send(Message::Text(
                json!({
                    "error": "CDP tunnel placeholder: agent byte-for-byte forwarding is deferred"
                })
                .to_string(),
            ))
            .await;
        let _ = socket.close().await;
    })
    .into_response()
}

async fn validate_cdp_token(state: &AppState, tenant_id: Uuid, raw: &str) -> Result<(), AppError> {
    let hash = hash_token(raw);
    let store = state.store.read().await;
    store
        .tokens
        .values()
        .find(|token| {
            token.tenant_id == tenant_id
                && token.token_type == TokenType::TenantCdpAccess
                && token.token_hash == hash
                && token.revoked_at.is_none()
        })
        .map(|_| ())
        .ok_or_else(|| AppError::unauthorized("invalid CDP token"))
}

fn ws_base_url(public_base_url: &str) -> String {
    public_base_url
        .replace("https://", "wss://")
        .replace("http://", "ws://")
}

async fn authorize(
    state: &AppState,
    headers: &HeaderMap,
    tenant_id: Option<Uuid>,
) -> Result<JwtClaims, AppError> {
    let token =
        bearer_token(headers).ok_or_else(|| AppError::unauthorized("missing bearer token"))?;
    let claims = decode_jwt(&state.config.jwt_secret, token)?;
    if let Some(tenant_id) = tenant_id {
        if !claims.tenants.iter().any(|tenant| tenant.id == tenant_id) {
            return Err(AppError::forbidden("token is not a member of this tenant"));
        }
    }
    Ok(claims)
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Ok(Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|err| anyhow::anyhow!(err.to_string()))?
        .to_string())
}

fn verify_password(password: &str, hash: &str) -> Result<(), AppError> {
    let parsed =
        PasswordHash::new(hash).map_err(|_| AppError::unauthorized("invalid credentials"))?;
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .map_err(|_| AppError::unauthorized("invalid credentials"))
}

fn hash_token(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

fn generate_token(prefix: &str) -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    format!("{}_{}", prefix, URL_SAFE_NO_PAD.encode(bytes))
}

fn issue_jwt(secret: &str, user: &User) -> Result<String, AppError> {
    let now = Utc::now();
    let claims = JwtClaims {
        sub: user.id.to_string(),
        email: user.email.clone(),
        tenants: user.tenants.clone(),
        iat: now.timestamp() as usize,
        exp: (now + ChronoDuration::hours(12)).timestamp() as usize,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|err| AppError::internal(format!("failed to issue jwt: {err}")))
}

fn decode_jwt(secret: &str, token: &str) -> Result<JwtClaims, AppError> {
    decode::<JwtClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|_| AppError::unauthorized("invalid bearer token"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JwtClaims {
    sub: String,
    email: String,
    tenants: Vec<TenantClaim>,
    iat: usize,
    exp: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TenantClaim {
    id: Uuid,
    role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Tenant {
    id: Uuid,
    name: String,
    slug: String,
}

#[derive(Debug, Clone)]
struct User {
    id: Uuid,
    email: String,
    display_name: String,
    password_hash: String,
    tenants: Vec<TenantClaim>,
}

#[derive(Debug, Serialize)]
struct UserResponse {
    id: Uuid,
    email: String,
    display_name: String,
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            email: user.email,
            display_name: user.display_name,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum TokenType {
    AgentRegistration,
    AgentRuntime,
    TenantCdpAccess,
}

#[derive(Debug, Clone)]
struct TokenRecord {
    id: Uuid,
    tenant_id: Uuid,
    token_type: TokenType,
    name: Option<String>,
    token_hash: String,
    token_prefix: String,
    created_by: Option<String>,
    revoked_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct TokenResponse {
    id: Uuid,
    tenant_id: Uuid,
    token_type: TokenType,
    name: Option<String>,
    token_prefix: String,
    created_by: Option<String>,
    revoked_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

impl From<&TokenRecord> for TokenResponse {
    fn from(token: &TokenRecord) -> Self {
        Self {
            id: token.id,
            tenant_id: token.tenant_id,
            token_type: token.token_type,
            name: token.name.clone(),
            token_prefix: token.token_prefix.clone(),
            created_by: token.created_by.clone(),
            revoked_at: token.revoked_at,
            created_at: token.created_at,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct AuditLog {
    id: Uuid,
    tenant_id: Uuid,
    actor_type: String,
    actor_id: String,
    action: String,
    source: String,
    browser_instance_id: Option<Uuid>,
    created_at: DateTime<Utc>,
}

impl AuditLog {
    fn new(
        tenant_id: Uuid,
        actor_type: impl Into<String>,
        actor_id: impl Into<String>,
        action: impl Into<String>,
        source: impl Into<String>,
        browser_instance_id: Option<Uuid>,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            tenant_id,
            actor_type: actor_type.into(),
            actor_id: actor_id.into(),
            action: action.into(),
            source: source.into(),
            browser_instance_id,
            created_at: Utc::now(),
        }
    }
}

#[derive(Debug, Error)]
enum AppError {
    #[error("{detail}")]
    Http {
        status: StatusCode,
        code: &'static str,
        detail: String,
    },
}

impl AppError {
    fn unauthorized(detail: impl Into<String>) -> Self {
        Self::Http {
            status: StatusCode::UNAUTHORIZED,
            code: "UNAUTHORIZED",
            detail: detail.into(),
        }
    }

    fn forbidden(detail: impl Into<String>) -> Self {
        Self::Http {
            status: StatusCode::FORBIDDEN,
            code: "FORBIDDEN",
            detail: detail.into(),
        }
    }

    fn not_found(resource: &'static str) -> Self {
        Self::Http {
            status: StatusCode::NOT_FOUND,
            code: "NOT_FOUND",
            detail: format!("{resource} not found"),
        }
    }

    fn internal(detail: impl Into<String>) -> Self {
        Self::Http {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "INTERNAL_ERROR",
            detail: detail.into(),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::Http {
                status,
                code,
                detail,
            } => (
                status,
                Json(json!({
                    "title": code,
                    "status": status.as_u16(),
                    "detail": detail
                })),
            )
                .into_response(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jwt_round_trips_tenant_claims() {
        let user = User {
            id: Uuid::now_v7(),
            email: "admin@example.com".to_string(),
            display_name: "Admin".to_string(),
            password_hash: "unused".to_string(),
            tenants: vec![TenantClaim {
                id: Uuid::now_v7(),
                role: "admin".to_string(),
            }],
        };

        let token = issue_jwt("test-secret", &user).unwrap();
        let claims = decode_jwt("test-secret", &token).unwrap();

        assert_eq!(claims.email, user.email);
        assert_eq!(claims.tenants[0].id, user.tenants[0].id);
    }

    #[test]
    fn token_hash_is_stable_and_not_plaintext() {
        let token = "jbr_cdp_secret";
        let hash = hash_token(token);

        assert_eq!(hash, hash_token(token));
        assert_ne!(hash, token);
    }
}
