use std::{collections::HashMap, sync::Arc, time::Instant};

use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::info;
use uuid::Uuid;

use jbrowser_shared::models::{AgentSummary, BrowserInstance};

use super::auth::crypto::{hash_password, hash_token};
use super::config::AppConfig;
use crate::db::repo;

#[derive(Clone)]
pub struct AppState {
    pub(crate) config: AppConfig,
    pub(crate) pool: sqlx::MySqlPool,
    pub(crate) agents: Arc<RwLock<HashMap<Uuid, AgentSummary>>>,
    pub(crate) browsers: Arc<RwLock<HashMap<Uuid, BrowserInstance>>>,
    pub(crate) agent_senders: Arc<RwLock<HashMap<Uuid, mpsc::Sender<String>>>>,
    pub(crate) browser_preview: Arc<RwLock<HashMap<Uuid, broadcast::Sender<Vec<u8>>>>>,
    pub(crate) browser_last_frame: Arc<RwLock<HashMap<Uuid, Vec<u8>>>>,
    pub(crate) browser_events: Arc<RwLock<HashMap<Uuid, broadcast::Sender<String>>>>,
    pub(crate) cdp_tunnel_senders: Arc<RwLock<HashMap<String, mpsc::Sender<String>>>>,
    /// browser_id → reset start time; while present, heartbeats must not overwrite status=Online
    pub(crate) pending_resets: Arc<RwLock<HashMap<Uuid, Instant>>>,
}

impl AppState {
    pub async fn new(config: AppConfig, pool: sqlx::MySqlPool) -> Self {
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
            agent_senders: Arc::new(RwLock::new(HashMap::new())),
            browser_preview: Arc::new(RwLock::new(HashMap::new())),
            browser_last_frame: Arc::new(RwLock::new(HashMap::new())),
            browser_events: Arc::new(RwLock::new(HashMap::new())),
            cdp_tunnel_senders: Arc::new(RwLock::new(HashMap::new())),
            pending_resets: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}
