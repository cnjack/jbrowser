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
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{Duration as ChronoDuration, Utc};
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

use crate::db::repo;

// ── Config ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub jwt_secret: String,
    pub database_url: String,
    pub public_base_url: String,
    pub demo_email: String,
    pub demo_password: String,
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

// ── AppState ────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AppState {
    config: AppConfig,
    pool: sqlx::MySqlPool,
    /// Runtime agent state (in-memory only, per AGENTS.md)
    agents: Arc<RwLock<HashMap<Uuid, AgentSummary>>>,
    /// Runtime browser state (in-memory only, per AGENTS.md)
    browsers: Arc<RwLock<HashMap<Uuid, BrowserInstance>>>,
    /// Global broadcast of raw video bytes (legacy)
    preview_tx: broadcast::Sender<Vec<u8>>,
    /// Per-agent command senders
    agent_senders: Arc<RwLock<HashMap<Uuid, mpsc::Sender<String>>>>,
    /// Per-browser video broadcast
    browser_preview: Arc<RwLock<HashMap<Uuid, broadcast::Sender<Vec<u8>>>>>,
    /// Per-browser last video frame cache
    browser_last_frame: Arc<RwLock<HashMap<Uuid, Vec<u8>>>>,
    /// Per-browser event broadcast
    browser_events: Arc<RwLock<HashMap<Uuid, broadcast::Sender<String>>>>,
    /// CDP tunnel senders
    cdp_tunnel_senders: Arc<RwLock<HashMap<String, mpsc::Sender<String>>>>,
}

impl AppState {
    pub async fn new(config: AppConfig, pool: sqlx::MySqlPool) -> Self {
        let (preview_tx, _) = broadcast::channel(64);

        // Seed demo user/tenant if not present
        let existing = repo::get_user_by_email(&pool, &config.demo_email)
            .await
            .ok()
            .flatten();
        if existing.is_none() {
            let tenant_id = Uuid::now_v7();
            let user_id = Uuid::now_v7();
            let password_hash = hash_password(&config.demo_password).expect("demo password hash");

            let _ = repo::create_tenant(&pool, tenant_id, "Default Tenant", "default").await;
            let _ = repo::create_user(
                &pool,
                user_id,
                &config.demo_email,
                &password_hash,
                "Demo Admin",
            )
            .await;
            let _ = repo::add_tenant_member(&pool, tenant_id, user_id, "admin").await;

            if let Some(raw_token) = &config.seed_agent_token {
                let token_id = Uuid::now_v7();
                let token_hash = hash_token(raw_token);
                let prefix: String = raw_token.chars().take(12).collect();
                let _ = repo::create_token(
                    &pool,
                    &repo::CreateToken {
                        id: token_id,
                        tenant_id,
                        token_type: "agent_registration",
                        name: Some("seed-agent-token"),
                        token_hash: &token_hash,
                        token_prefix: &prefix,
                        created_by: Some(&user_id.to_string()),
                    },
                )
                .await;
            }
            info!(email = %config.demo_email, "seeded demo user + tenant");
        } else if let Some(raw_token) = &config.seed_agent_token {
            let token_hash = hash_token(raw_token);
            if repo::get_token_by_hash(&pool, &token_hash)
                .await
                .ok()
                .flatten()
                .is_none()
            {
                if let Some(user_row) = existing {
                    let user_id = Uuid::parse_str(&user_row.id).unwrap();
                    let tenant_rows = repo::get_user_tenants(&pool, user_id)
                        .await
                        .unwrap_or_default();
                    if let Some(t) = tenant_rows.first() {
                        let tenant_id = Uuid::parse_str(&t.id).unwrap();
                        let token_id = Uuid::now_v7();
                        let prefix: String = raw_token.chars().take(12).collect();
                        let _ = repo::create_token(
                            &pool,
                            &repo::CreateToken {
                                id: token_id,
                                tenant_id,
                                token_type: "agent_registration",
                                name: Some("seed-agent-token"),
                                token_hash: &token_hash,
                                token_prefix: &prefix,
                                created_by: Some(&user_id.to_string()),
                            },
                        )
                        .await;
                    }
                }
            }
        }

        Self {
            config,
            pool,
            agents: Arc::new(RwLock::new(HashMap::new())),
            browsers: Arc::new(RwLock::new(HashMap::new())),
            preview_tx,
            agent_senders: Arc::new(RwLock::new(HashMap::new())),
            browser_preview: Arc::new(RwLock::new(HashMap::new())),
            browser_last_frame: Arc::new(RwLock::new(HashMap::new())),
            browser_events: Arc::new(RwLock::new(HashMap::new())),
            cdp_tunnel_senders: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

// ── Router ──────────────────────────────────────────────────────────────

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/metrics", get(metrics))
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/auth/signup", post(signup))
        .route("/api/v1/auth/me", get(me))
        .route("/api/v1/auth/change-password", post(change_password))
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
            post(revoke_token_handler),
        )
        .route(
            "/api/v1/tenants/:tenant_id/tokens/cdp/:token_id/rotate",
            post(rotate_token),
        )
        .route(
            "/api/v1/tenants/:tenant_id/agent-registration-tokens",
            get(list_agent_registration_tokens).post(create_agent_registration_token),
        )
        .route(
            "/api/v1/tenants/:tenant_id/agent-registration-tokens/:token_id/revoke",
            post(revoke_token_handler),
        )
        .route(
            "/api/v1/tenants/:tenant_id/audit-logs",
            get(list_audit_logs),
        )
        .route(
            "/api/v1/tenants/:tenant_id/invitations",
            get(list_invitations).post(create_invitation),
        )
        .route(
            "/api/v1/tenants/:tenant_id/invitations/:invitation_id",
            axum::routing::delete(delete_invitation_handler),
        )
        .route(
            "/api/v1/invitations/:token/validate",
            get(validate_invitation),
        )
        .route("/api/v1/invitations/:token/accept", post(accept_invitation))
        .route(
            "/api/v1/tenants/:tenant_id/members",
            get(list_members_handler),
        )
        .route(
            "/api/v1/tenants/:tenant_id/members/leave",
            post(leave_tenant),
        )
        .route(
            "/api/v1/tenants/:tenant_id/members/:user_id",
            axum::routing::patch(update_member_role_handler).delete(remove_member),
        )
        .route(
            "/api/v1/tenants/:tenant_id",
            axum::routing::patch(update_tenant_handler).delete(delete_tenant_handler),
        )
        .route("/api/v1/tenants", post(create_tenant_handler))
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
            get(cdp_ws_tunnel),
        )
        .route(
            "/cdp/tenants/:tenant_id/browser-instances/:browser_id/devtools/page/:target_id",
            get(cdp_ws_tunnel),
        )
        .fallback_service(
            ServeDir::new("frontend/dist").fallback(ServeFile::new("frontend/dist/index.html")),
        )
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}

// ── Health / Ready / Metrics ────────────────────────────────────────────

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

async fn ready(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "checks": {
            "database_url_configured": !state.config.database_url.is_empty(),
            "state": "mysql_hybrid"
        }
    }))
}

async fn metrics(State(state): State<AppState>) -> String {
    let browsers = state.browsers.read().await;
    let online = browsers
        .values()
        .filter(|browser| browser.status == BrowserStatus::Online)
        .count();
    format!(
        "# HELP browser_instances_online Online browser instances\n# TYPE browser_instances_online gauge\nbrowser_instances_online {}\n",
        online
    )
}

// ── Auth: Login ─────────────────────────────────────────────────────────

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
    let user_row = repo::get_user_by_email(&state.pool, &req.email)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?
        .ok_or(AppError::unauthorized("invalid credentials"))?;

    verify_password(&req.password, &user_row.password_hash)?;

    let user_id = Uuid::parse_str(&user_row.id).unwrap();
    let tenant_rows = repo::get_user_tenants(&state.pool, user_id)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;

    let tenants_claim: Vec<TenantClaim> = tenant_rows
        .iter()
        .map(|t| TenantClaim {
            id: Uuid::parse_str(&t.id).unwrap(),
            role: t.role.clone(),
        })
        .collect();

    let user = User {
        id: user_id,
        email: user_row.email,
        display_name: user_row.display_name.unwrap_or_default(),
        password_hash: user_row.password_hash,
        tenants: tenants_claim.clone(),
    };
    let access_token = issue_jwt(&state.config.jwt_secret, &user)?;
    let tenants: Vec<Tenant> = tenant_rows
        .iter()
        .map(|t| Tenant {
            id: Uuid::parse_str(&t.id).unwrap(),
            name: t.name.clone(),
            slug: t.slug.clone(),
            role: t.role.clone(),
        })
        .collect();

    if let Some(tc) = tenants_claim.first() {
        let _ = repo::create_audit_log(
            &state.pool,
            &repo::CreateAuditLog {
                id: Uuid::now_v7(),
                tenant_id: tc.id,
                actor_type: "user",
                actor_id: &user_id.to_string(),
                action: "user.login",
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

    Ok(Json(LoginResponse {
        access_token,
        token_type: "Bearer",
        user: UserResponse {
            id: user_id,
            email: user.email,
            display_name: user.display_name,
        },
        tenants,
    }))
}

// ── Auth: Signup ─────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct SignupRequest {
    email: String,
    password: String,
    display_name: String,
}

async fn signup(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
    Json(req): Json<SignupRequest>,
) -> Result<Json<LoginResponse>, AppError> {
    if req.password.len() < 8 {
        return Err(AppError::bad_request(
            "password must be at least 8 characters",
        ));
    }
    if repo::get_user_by_email(&state.pool, &req.email)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?
        .is_some()
    {
        return Err(AppError::bad_request("email already registered"));
    }

    let user_id = Uuid::now_v7();
    let pw_hash = hash_password(&req.password).map_err(|e| AppError::internal(e.to_string()))?;
    repo::create_user(
        &state.pool,
        user_id,
        &req.email,
        &pw_hash,
        &req.display_name,
    )
    .await
    .map_err(|e| AppError::internal(e.to_string()))?;

    let invite_token = params.get("invite_token");
    let mut tenants_claim = Vec::new();
    let mut tenants_resp = Vec::new();

    if let Some(raw_token) = invite_token {
        let token_hash = hash_token(raw_token);
        if let Some(inv) = repo::get_invitation_by_token_hash(&state.pool, &token_hash)
            .await
            .map_err(|e| AppError::internal(e.to_string()))?
        {
            let tenant_id = Uuid::parse_str(&inv.tenant_id).unwrap();
            repo::add_tenant_member(&state.pool, tenant_id, user_id, &inv.role)
                .await
                .map_err(|e| AppError::internal(e.to_string()))?;
            let inv_id = Uuid::parse_str(&inv.id).unwrap();
            repo::accept_invitation(&state.pool, inv_id, user_id)
                .await
                .map_err(|e| AppError::internal(e.to_string()))?;
            if let Some(t) = repo::get_tenant(&state.pool, tenant_id)
                .await
                .map_err(|e| AppError::internal(e.to_string()))?
            {
                tenants_claim.push(TenantClaim {
                    id: tenant_id,
                    role: inv.role.clone(),
                });
                tenants_resp.push(Tenant {
                    id: tenant_id,
                    name: t.name,
                    slug: t.slug,
                    role: inv.role.clone(),
                });
            }
        } else {
            return Err(AppError::bad_request("invalid or expired invitation"));
        }
    } else {
        let tenant_id = Uuid::now_v7();
        let slug = generate_slug(&req.display_name);
        let tenant_name = format!("{}'s Workspace", req.display_name);
        repo::create_tenant(&state.pool, tenant_id, &tenant_name, &slug)
            .await
            .map_err(|e| AppError::internal(e.to_string()))?;
        repo::add_tenant_member(&state.pool, tenant_id, user_id, "admin")
            .await
            .map_err(|e| AppError::internal(e.to_string()))?;
        tenants_claim.push(TenantClaim {
            id: tenant_id,
            role: "admin".to_string(),
        });
        tenants_resp.push(Tenant {
            id: tenant_id,
            name: tenant_name,
            slug,
            role: "admin".to_string(),
        });
    }

    let user = User {
        id: user_id,
        email: req.email,
        display_name: req.display_name,
        password_hash: pw_hash,
        tenants: tenants_claim,
    };
    let access_token = issue_jwt(&state.config.jwt_secret, &user)?;
    Ok(Json(LoginResponse {
        access_token,
        token_type: "Bearer",
        user: UserResponse {
            id: user_id,
            email: user.email,
            display_name: user.display_name,
        },
        tenants: tenants_resp,
    }))
}

fn generate_slug(name: &str) -> String {
    let base: String = name
        .chars()
        .filter_map(|c| {
            if c.is_alphanumeric() {
                Some(c.to_ascii_lowercase())
            } else if c == ' ' || c == '-' {
                Some('-')
            } else {
                None
            }
        })
        .collect();
    let base = base.trim_matches('-').to_string();
    let suffix: String = (0..3)
        .map(|_| {
            let idx = rand::random::<u8>() % 36;
            if idx < 10 {
                (b'0' + idx) as char
            } else {
                (b'a' + idx - 10) as char
            }
        })
        .collect();
    if base.is_empty() {
        format!("workspace-{suffix}")
    } else {
        format!("{base}-{suffix}")
    }
}

// ── Auth: Me ────────────────────────────────────────────────────────────

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

// ── Auth: Change Password ───────────────────────────────────────────────

#[derive(Deserialize)]
struct ChangePasswordRequest {
    current_password: String,
    new_password: String,
}

async fn change_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<ChangePasswordRequest>,
) -> Result<Json<Value>, AppError> {
    let claims = authorize(&state, &headers, None).await?;
    if req.new_password.len() < 8 {
        return Err(AppError::bad_request(
            "password must be at least 8 characters",
        ));
    }
    let user_id = Uuid::parse_str(&claims.sub).unwrap();
    let user = repo::get_user_by_id(&state.pool, user_id)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?
        .ok_or(AppError::not_found("user"))?;
    verify_password(&req.current_password, &user.password_hash)?;
    let new_hash =
        hash_password(&req.new_password).map_err(|e| AppError::internal(e.to_string()))?;
    repo::update_user_password(&state.pool, user_id, &new_hash)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    Ok(Json(json!({"message": "password changed"})))
}

// ── Browser Instances ───────────────────────────────────────────────────

async fn list_browsers(
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

async fn get_browser(
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

async fn reset_browser(
    State(state): State<AppState>,
    Path((tenant_id, browser_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    let claims = authorize(&state, &headers, Some(tenant_id)).await?;
    let (snapshot, agent_id) = {
        let mut browsers = state.browsers.write().await;
        let browser = browsers
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
        (snapshot, agent_id)
    };

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

async fn delete_browser(
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

// ── Agents ──────────────────────────────────────────────────────────────

async fn list_agents(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    authorize(&state, &headers, Some(tenant_id)).await?;
    let agents = state.agents.read().await;
    let data: Vec<_> = agents
        .values()
        .filter(|agent| agent.tenant_id == tenant_id)
        .cloned()
        .collect();
    Ok(Json(json!({ "data": data })))
}

async fn get_agent(
    State(state): State<AppState>,
    Path((tenant_id, agent_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    authorize(&state, &headers, Some(tenant_id)).await?;
    let agents = state.agents.read().await;
    let agent = agents
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

// ── Tokens ──────────────────────────────────────────────────────────────

async fn list_cdp_tokens(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    authorize(&state, &headers, Some(tenant_id)).await?;
    let rows = repo::list_tokens(&state.pool, tenant_id, "tenant_cdp_access")
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    let data: Vec<_> = rows.iter().map(token_row_to_response).collect();
    Ok(Json(json!({ "data": data })))
}

async fn list_agent_registration_tokens(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    authorize(&state, &headers, Some(tenant_id)).await?;
    let rows = repo::list_tokens(&state.pool, tenant_id, "agent_registration")
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    let data: Vec<_> = rows.iter().map(token_row_to_response).collect();
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
    create_token_impl(
        state,
        tenant_id,
        headers,
        "tenant_cdp_access",
        "jbr_cdp",
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
    create_token_impl(
        state,
        tenant_id,
        headers,
        "agent_registration",
        "jbr_reg",
        name,
    )
    .await
}

async fn create_token_impl(
    state: AppState,
    tenant_id: Uuid,
    headers: HeaderMap,
    token_type: &str,
    prefix: &str,
    name: Option<String>,
) -> Result<Json<Value>, AppError> {
    let claims = require_admin(&state, &headers, tenant_id).await?;
    let raw = generate_token(prefix);
    let id = Uuid::now_v7();
    let thash = hash_token(&raw);
    let tprefix: String = raw.chars().take(12).collect();

    repo::create_token(
        &state.pool,
        &repo::CreateToken {
            id,
            tenant_id,
            token_type,
            name: name.as_deref(),
            token_hash: &thash,
            token_prefix: &tprefix,
            created_by: Some(&claims.sub),
        },
    )
    .await
    .map_err(|e| AppError::internal(e.to_string()))?;

    let _ = repo::create_audit_log(
        &state.pool,
        &repo::CreateAuditLog {
            id: Uuid::now_v7(),
            tenant_id,
            actor_type: "user",
            actor_id: &claims.sub,
            action: "token.created",
            source: "web_ui",
            resource_type: None,
            resource_id: None,
            browser_instance_id: None,
            tab_id: None,
            metadata: None,
        },
    )
    .await;

    let response = json!({
        "data": {
            "id": id,
            "tenant_id": tenant_id,
            "token_type": token_type,
            "name": name,
            "token_prefix": tprefix,
            "created_by": claims.sub,
            "revoked_at": null,
            "created_at": Utc::now()
        },
        "token": raw
    });
    Ok(Json(response))
}

async fn revoke_token_handler(
    State(state): State<AppState>,
    Path((tenant_id, token_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    let claims = require_admin(&state, &headers, tenant_id).await?;
    let revoked = repo::revoke_token(&state.pool, tenant_id, token_id)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    if !revoked {
        return Err(AppError::not_found("token"));
    }
    let _ = repo::create_audit_log(
        &state.pool,
        &repo::CreateAuditLog {
            id: Uuid::now_v7(),
            tenant_id,
            actor_type: "user",
            actor_id: &claims.sub,
            action: "token.revoked",
            source: "web_ui",
            resource_type: None,
            resource_id: None,
            browser_instance_id: None,
            tab_id: None,
            metadata: None,
        },
    )
    .await;
    Ok(Json(json!({ "data": { "id": token_id, "revoked": true } })))
}

async fn rotate_token(
    State(state): State<AppState>,
    Path((tenant_id, token_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    let claims = require_admin(&state, &headers, tenant_id).await?;
    let _ = repo::revoke_token(&state.pool, tenant_id, token_id)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    let raw = generate_token("jbr_cdp");
    let new_id = Uuid::now_v7();
    let thash = hash_token(&raw);
    let tprefix: String = raw.chars().take(12).collect();
    repo::create_token(
        &state.pool,
        &repo::CreateToken {
            id: new_id,
            tenant_id,
            token_type: "tenant_cdp_access",
            name: None,
            token_hash: &thash,
            token_prefix: &tprefix,
            created_by: Some(&claims.sub),
        },
    )
    .await
    .map_err(|e| AppError::internal(e.to_string()))?;

    let _ = repo::create_audit_log(
        &state.pool,
        &repo::CreateAuditLog {
            id: Uuid::now_v7(),
            tenant_id,
            actor_type: "user",
            actor_id: &claims.sub,
            action: "token.rotated",
            source: "web_ui",
            resource_type: None,
            resource_id: None,
            browser_instance_id: None,
            tab_id: None,
            metadata: None,
        },
    )
    .await;

    Ok(Json(json!({
        "data": {
            "id": new_id,
            "tenant_id": tenant_id,
            "token_type": "tenant_cdp_access",
            "token_prefix": tprefix,
            "created_by": claims.sub,
            "revoked_at": null,
            "created_at": Utc::now()
        },
        "token": raw
    })))
}

// ── Audit Logs ──────────────────────────────────────────────────────────

async fn list_audit_logs(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    require_admin(&state, &headers, tenant_id).await?;
    let rows = repo::list_audit_logs(&state.pool, tenant_id, 200)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    let data: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.id,
                "tenant_id": r.tenant_id,
                "actor_type": r.actor_type,
                "actor_id": r.actor_id,
                "action": r.action,
                "resource_type": r.resource_type,
                "resource_id": r.resource_id,
                "source": r.source,
                "metadata": r.metadata,
                "created_at": r.created_at
            })
        })
        .collect();
    Ok(Json(json!({ "data": data })))
}

// ── Agent Registration ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct AgentRegisterRequest {
    registration_token: String,
    name: Option<String>,
    browser_type: Option<String>,
    browser_version: Option<String>,
    /// If the agent already has a stable identity, send it back.
    /// The control plane will reuse the same IDs and only refresh the runtime token.
    agent_id: Option<String>,
    browser_instance_id: Option<String>,
}

async fn agent_register(
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

    // ── Re-registration: agent already has a stable identity ────────────────
    if let (Some(existing_agent_id_str), Some(existing_browser_id_str)) =
        (req.agent_id.as_deref(), req.browser_instance_id.as_deref())
    {
        if let (Ok(existing_agent_id), Ok(existing_browser_id)) = (
            Uuid::parse_str(existing_agent_id_str),
            Uuid::parse_str(existing_browser_id_str),
        ) {
            // Verify the agent exists in DB and belongs to this tenant
            if let Ok(Some(agent_row)) = repo::get_agent(&state.pool, tenant_id, existing_agent_id).await {
                let agent_name = agent_row.name.clone().unwrap_or_else(|| format!("agent-{existing_agent_id}"));

                // Refresh runtime token only
                let _ = repo::update_agent_runtime_token(
                    &state.pool,
                    existing_agent_id,
                    &hash_token(&runtime_token),
                ).await;
                let _ = repo::replace_agent_runtime_token(
                    &state.pool,
                    existing_agent_id,
                    tenant_id,
                    &hash_token(&runtime_token),
                    &runtime_token.chars().take(12).collect::<String>(),
                ).await;

                // Reload into memory
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
                };
                state.agents.write().await.insert(existing_agent_id, agent);
                state.browsers.write().await.insert(existing_browser_id, browser);

                info!(%existing_agent_id, "agent re-registered with stable identity");
                return Ok(Json(json!({
                    "agent_id": existing_agent_id,
                    "browser_instance_id": existing_browser_id,
                    "agent_runtime_token": runtime_token
                })));
            }
        }
    }

    // ── First-time registration: create new identity ─────────────────────────
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

// ── Agent WebSocket ─────────────────────────────────────────────────────

async fn ws_agent(
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
            // Control plane may have restarted — try to restore agent from DB.
            match repo::get_agent(&state.pool, tenant_id, hint_id).await {
                Ok(Some(row)) => {
                    let bid = Uuid::parse_str(&row.browser_instance_id).unwrap_or_else(|_| Uuid::now_v7());
                    let agent_name = row.name.clone().unwrap_or_else(|| format!("agent-{hint_id}"));
                    let browser_row = repo::get_browser_instance(&state.pool, tenant_id, bid).await.ok().flatten();
                    let (btype, bver) = browser_row.as_ref().map(|b| (b.browser_type.clone(), b.browser_version.clone().unwrap_or_else(|| "unknown".to_string()))).unwrap_or_else(|| ("chromium".to_string(), "unknown".to_string()));
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

    let (bcast_tx, _) = broadcast::channel::<Vec<u8>>(128);
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
                            let _ = state.preview_tx.send(bytes.clone());
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

    // Cleanup on disconnect — in-memory only
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
    {
        let mut agents = state.agents.write().await;
        if let Some(agent) = agents.get_mut(&agent_id) {
            agent.status = AgentStatus::Online;
            agent.last_heartbeat_at = Some(now.clone());
        }
    }
    {
        let mut browsers = state.browsers.write().await;
        if let Some(browser) = browsers.get_mut(&browser_id) {
            browser.status = BrowserStatus::Online;
            browser.agent_status = AgentStatus::Online;
            browser.last_heartbeat_at = Some(now);
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
}

// ── Control WebSocket ───────────────────────────────────────────────────

async fn ws_control(State(state): State<AppState>, ws: WebSocketUpgrade) -> Response {
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
        "input.event" | "tab.command" | "browser.reset" | "navigate.url" | "navigate.back"
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
                            "navigate.url" | "navigate.back" | "navigate.forward"
                            | "navigate.reload" => "navigate.requested",
                            _ => "browser.reset_requested",
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

// ── CDP Proxy ───────────────────────────────────────────────────────────

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

async fn cdp_json_list(
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

async fn cdp_ws_tunnel(
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

    let (response_tx, mut response_rx) = mpsc::channel::<String>(256);

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
                        let senders = state.agent_senders.read().await;
                        if let Some(tx) = senders.get(&agent_id) {
                            let tunnel_msg = json!({
                                "type": "cdp.tunnel.message",
                                "payload": {
                                    "session_id": session_id,
                                    "data": text
                                }
                            });
                            if tx.try_send(tunnel_msg.to_string()).is_err() {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
            Some(response) = response_rx.recv() => {
                if ws_tx.send(Message::Text(response)).await.is_err() {
                    break;
                }
            }
        }
    }

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

// ── Invitation Handlers ─────────────────────────────────────────────────

#[derive(Deserialize)]
struct CreateInvitationRequest {
    role: Option<String>,
}

async fn create_invitation(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
    headers: HeaderMap,
    Json(req): Json<CreateInvitationRequest>,
) -> Result<Json<Value>, AppError> {
    let claims = require_admin(&state, &headers, tenant_id).await?;
    let role = req.role.unwrap_or_else(|| "member".to_string());
    if role != "admin" && role != "member" {
        return Err(AppError::bad_request("role must be admin or member"));
    }
    let id = Uuid::now_v7();
    let raw_token = generate_token("jbr_inv");
    let token_hash = hash_token(&raw_token);
    let prefix: String = raw_token.chars().take(8).collect();
    let expires_at = Utc::now() + ChronoDuration::days(7);
    let user_id = Uuid::parse_str(&claims.sub).unwrap();
    repo::create_invitation(
        &state.pool,
        &repo::CreateInvitation {
            id,
            tenant_id,
            token_hash: &token_hash,
            token_prefix: &prefix,
            role: &role,
            created_by: user_id,
            expires_at,
        },
    )
    .await
    .map_err(|e| AppError::internal(e.to_string()))?;
    let invite_link = format!("{}/invite/{}", state.config.public_base_url, raw_token);
    Ok(Json(json!({
        "id": id,
        "invite_link": invite_link,
        "token": raw_token,
        "role": role,
        "created_by": claims.sub,
        "expires_at": expires_at
    })))
}

async fn list_invitations(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    require_admin(&state, &headers, tenant_id).await?;
    let rows = repo::list_invitations_by_tenant(&state.pool, tenant_id)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    Ok(Json(json!({
        "data": rows.iter().map(|r| json!({
            "id": r.id,
            "role": r.role,
            "token_prefix": r.token_prefix,
            "created_by": r.created_by,
            "accepted_by": r.accepted_by,
            "accepted_at": r.accepted_at,
            "expires_at": r.expires_at,
            "created_at": r.created_at
        })).collect::<Vec<_>>()
    })))
}

async fn delete_invitation_handler(
    State(state): State<AppState>,
    Path((tenant_id, invitation_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<StatusCode, AppError> {
    require_admin(&state, &headers, tenant_id).await?;
    let deleted = repo::delete_invitation(&state.pool, tenant_id, invitation_id)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::not_found("invitation"))
    }
}

async fn validate_invitation(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<Json<Value>, AppError> {
    let token_hash = hash_token(&token);
    let inv = repo::get_invitation_by_token_hash(&state.pool, &token_hash)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    match inv {
        Some(row) => Ok(Json(json!({
            "valid": true,
            "tenant_name": row.tenant_name,
            "role": row.role
        }))),
        None => Ok(Json(json!({ "valid": false }))),
    }
}

async fn accept_invitation(
    State(state): State<AppState>,
    Path(token): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    let claims = authorize(&state, &headers, None).await?;
    let user_id = Uuid::parse_str(&claims.sub).unwrap();
    let token_hash = hash_token(&token);
    let inv = repo::get_invitation_by_token_hash(&state.pool, &token_hash)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?
        .ok_or(AppError::bad_request("invalid or expired invitation"))?;
    let tenant_id = Uuid::parse_str(&inv.tenant_id).unwrap();
    let inv_id = Uuid::parse_str(&inv.id).unwrap();
    if repo::get_member_role(&state.pool, tenant_id, user_id)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?
        .is_some()
    {
        return Err(AppError::bad_request("already a member of this tenant"));
    }
    repo::add_tenant_member(&state.pool, tenant_id, user_id, &inv.role)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    repo::accept_invitation(&state.pool, inv_id, user_id)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    let tenant_rows = repo::get_user_tenants(&state.pool, user_id)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    let tenants_claim: Vec<TenantClaim> = tenant_rows
        .iter()
        .map(|t| TenantClaim {
            id: Uuid::parse_str(&t.id).unwrap(),
            role: t.role.clone(),
        })
        .collect();
    let user_row = repo::get_user_by_id(&state.pool, user_id)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?
        .unwrap();
    let user = User {
        id: user_id,
        email: user_row.email,
        display_name: user_row.display_name.unwrap_or_default(),
        password_hash: user_row.password_hash,
        tenants: tenants_claim,
    };
    let access_token = issue_jwt(&state.config.jwt_secret, &user)?;
    Ok(Json(json!({ "access_token": access_token })))
}

// ── Member Handlers ─────────────────────────────────────────────────────

async fn list_members_handler(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    authorize(&state, &headers, Some(tenant_id)).await?;
    let rows = repo::list_members(&state.pool, tenant_id)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    Ok(Json(json!({
        "members": rows.iter().map(|r| json!({
            "user_id": r.user_id,
            "email": r.email,
            "display_name": r.display_name,
            "role": r.role,
            "joined_at": r.joined_at
        })).collect::<Vec<_>>()
    })))
}

#[derive(Deserialize)]
struct UpdateRoleRequest {
    role: String,
}

async fn update_member_role_handler(
    State(state): State<AppState>,
    Path((tenant_id, target_user_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(req): Json<UpdateRoleRequest>,
) -> Result<Json<Value>, AppError> {
    let claims = require_admin(&state, &headers, tenant_id).await?;
    if req.role != "admin" && req.role != "member" {
        return Err(AppError::bad_request("role must be admin or member"));
    }
    let caller_id = Uuid::parse_str(&claims.sub).unwrap();
    if caller_id == target_user_id {
        return Err(AppError::bad_request("cannot change your own role"));
    }
    if req.role == "member" {
        let admin_count = repo::count_admins(&state.pool, tenant_id)
            .await
            .map_err(|e| AppError::internal(e.to_string()))?;
        let current_role = repo::get_member_role(&state.pool, tenant_id, target_user_id)
            .await
            .map_err(|e| AppError::internal(e.to_string()))?;
        if current_role.as_deref() == Some("admin") && admin_count <= 1 {
            return Err(AppError::bad_request("cannot demote the last admin"));
        }
    }
    repo::update_member_role(&state.pool, tenant_id, target_user_id, &req.role)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    Ok(Json(json!({"message": "role updated"})))
}

async fn remove_member(
    State(state): State<AppState>,
    Path((tenant_id, target_user_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<StatusCode, AppError> {
    let claims = require_admin(&state, &headers, tenant_id).await?;
    let caller_id = Uuid::parse_str(&claims.sub).unwrap();
    if caller_id == target_user_id {
        return Err(AppError::bad_request(
            "cannot remove yourself, use leave instead",
        ));
    }
    let removed = repo::remove_tenant_member(&state.pool, tenant_id, target_user_id)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    if removed {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::not_found("member"))
    }
}

async fn leave_tenant(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, AppError> {
    let claims = authorize(&state, &headers, Some(tenant_id)).await?;
    let user_id = Uuid::parse_str(&claims.sub).unwrap();
    let role = repo::get_member_role(&state.pool, tenant_id, user_id)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?
        .ok_or(AppError::not_found("membership"))?;
    if role == "admin" {
        let count = repo::count_admins(&state.pool, tenant_id)
            .await
            .map_err(|e| AppError::internal(e.to_string()))?;
        if count <= 1 {
            return Err(AppError::bad_request(
                "cannot leave: you are the last admin",
            ));
        }
    }
    repo::remove_tenant_member(&state.pool, tenant_id, user_id)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

// ── Tenant Management ───────────────────────────────────────────────────

#[derive(Deserialize)]
struct UpdateTenantRequest {
    name: String,
}

// ── Tenant: Create ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct CreateTenantRequest {
    name: String,
}

async fn create_tenant_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateTenantRequest>,
) -> Result<Json<Value>, AppError> {
    let name = req.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::bad_request("tenant name cannot be empty"));
    }
    let claims = authorize(&state, &headers, None).await?;
    let user_id = Uuid::parse_str(&claims.sub).unwrap();
    let tenant_id = Uuid::now_v7();
    let slug = generate_slug(&name);
    repo::create_tenant(&state.pool, tenant_id, &name, &slug)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    repo::add_tenant_member(&state.pool, tenant_id, user_id, "admin")
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;

    // Return fresh token with the new tenant included
    let tenant_rows = repo::get_user_tenants(&state.pool, user_id)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    let tenants_claim: Vec<TenantClaim> = tenant_rows
        .iter()
        .map(|t| TenantClaim { id: Uuid::parse_str(&t.id).unwrap(), role: t.role.clone() })
        .collect();
    let user_row = repo::get_user_by_id(&state.pool, user_id)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?
        .ok_or(AppError::unauthorized("user not found"))?;
    let user = User {
        id: user_id,
        email: user_row.email.clone(),
        display_name: user_row.display_name.clone().unwrap_or_default(),
        password_hash: user_row.password_hash.clone(),
        tenants: tenants_claim,
    };
    let access_token = issue_jwt(&state.config.jwt_secret, &user)?;
    let tenants_resp: Vec<Tenant> = tenant_rows
        .iter()
        .map(|t| Tenant {
            id: Uuid::parse_str(&t.id).unwrap(),
            name: t.name.clone(),
            slug: t.slug.clone(),
            role: t.role.clone(),
        })
        .collect();

    Ok(Json(json!({
        "access_token": access_token,
        "token_type": "Bearer",
        "tenant": {
            "id": tenant_id,
            "name": name,
            "slug": slug,
            "role": "admin"
        },
        "tenants": tenants_resp
    })))
}

async fn update_tenant_handler(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
    headers: HeaderMap,
    Json(req): Json<UpdateTenantRequest>,
) -> Result<Json<Value>, AppError> {
    require_admin(&state, &headers, tenant_id).await?;
    repo::update_tenant(&state.pool, tenant_id, &req.name)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    Ok(Json(json!({"message": "tenant updated"})))
}

async fn delete_tenant_handler(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, AppError> {
    require_admin(&state, &headers, tenant_id).await?;
    {
        let browsers = state.browsers.read().await;
        let browser_ids: Vec<Uuid> = browsers
            .values()
            .filter(|b| b.tenant_id == tenant_id)
            .map(|b| b.id)
            .collect();
        let agent_ids: Vec<Uuid> = browsers
            .values()
            .filter(|b| b.tenant_id == tenant_id)
            .map(|b| b.agent_id)
            .collect();
        drop(browsers);
        state
            .browsers
            .write()
            .await
            .retain(|_, b| b.tenant_id != tenant_id);
        state
            .agents
            .write()
            .await
            .retain(|_, a| a.tenant_id != tenant_id);
        for bid in &browser_ids {
            state.browser_preview.write().await.remove(bid);
            state.browser_last_frame.write().await.remove(bid);
            state.browser_events.write().await.remove(bid);
        }
        for aid in &agent_ids {
            state.agent_senders.write().await.remove(aid);
        }
    }
    repo::delete_tenant(&state.pool, tenant_id)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

// ── Auth / RBAC Helpers ─────────────────────────────────────────────────

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

async fn require_admin(
    state: &AppState,
    headers: &HeaderMap,
    tenant_id: Uuid,
) -> Result<JwtClaims, AppError> {
    let claims = authorize(state, headers, Some(tenant_id)).await?;
    let is_admin = claims
        .tenants
        .iter()
        .any(|t| t.id == tenant_id && t.role == "admin");
    if !is_admin {
        return Err(AppError::forbidden("admin role required"));
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

// ── Crypto Helpers ──────────────────────────────────────────────────────

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

// ── Token Row → Response helper ─────────────────────────────────────────

fn token_row_to_response(row: &repo::TokenRow) -> Value {
    json!({
        "id": row.id,
        "tenant_id": row.tenant_id,
        "token_type": row.token_type,
        "name": row.name,
        "token_prefix": row.token_prefix,
        "created_by": row.created_by,
        "revoked_at": row.revoked_at,
        "created_at": row.created_at
    })
}

// ── Domain Types ────────────────────────────────────────────────────────

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
    role: String,
}

#[derive(Debug, Clone)]
struct User {
    id: Uuid,
    email: String,
    display_name: String,
    #[allow(dead_code)]
    password_hash: String,
    tenants: Vec<TenantClaim>,
}

#[derive(Debug, Serialize)]
struct UserResponse {
    id: Uuid,
    email: String,
    display_name: String,
}

// ── Error Type ──────────────────────────────────────────────────────────

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

    fn bad_request(detail: impl Into<String>) -> Self {
        Self::Http {
            status: StatusCode::BAD_REQUEST,
            code: "BAD_REQUEST",
            detail: detail.into(),
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

// ── Tests ───────────────────────────────────────────────────────────────

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

    #[test]
    fn generate_slug_basic() {
        let slug = generate_slug("Alice Smith");
        assert!(slug.starts_with("alice-smith-"));
        assert_eq!(slug.len(), "alice-smith-".len() + 3);
    }

    #[test]
    fn generate_slug_empty() {
        let slug = generate_slug("");
        assert!(slug.starts_with("workspace-"));
    }

    #[test]
    fn generate_slug_special_chars() {
        let slug = generate_slug("Hello! @World#");
        assert!(slug.starts_with("hello-world-"));
    }
}
