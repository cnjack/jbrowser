use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct TenantRow {
    pub id: String,
    pub name: String,
    pub slug: String,
}

#[derive(Debug, Clone)]
pub struct UserRow {
    pub id: String,
    pub email: String,
    pub password_hash: String,
    pub display_name: Option<String>,
    pub is_platform_admin: bool,
}

#[derive(Debug, Clone)]
pub struct MemberRow {
    pub user_id: String,
    pub email: String,
    pub display_name: Option<String>,
    pub role: String,
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct UserTenantRow {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub role: String,
}

#[derive(Debug, Clone)]
pub struct TokenRow {
    pub id: String,
    pub tenant_id: String,
    pub token_type: String,
    pub name: Option<String>,
    pub token_hash: String,
    pub token_prefix: Option<String>,
    pub created_by: Option<String>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct AgentRow {
    pub id: String,
    pub tenant_id: String,
    pub browser_instance_id: String,
    pub name: Option<String>,
    pub runtime_token_hash: String,
    pub status: String,
    pub last_heartbeat_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct BrowserRow {
    pub id: String,
    pub tenant_id: String,
    pub agent_id: String,
    pub browser_type: String,
    pub browser_version: Option<String>,
    pub status: String,
    pub active_tab_id: Option<String>,
    pub tabs_snapshot: Option<String>,
    pub viewport_width: i32,
    pub viewport_height: i32,
    pub user_agent: Option<String>,
    pub device_scale_factor: f64,
    pub timezone: Option<String>,
    pub locale: Option<String>,
    pub stealth_level: String,
}

#[derive(Debug, Clone)]
pub struct InvitationRow {
    pub id: String,
    pub tenant_id: String,
    pub token_hash: String,
    pub token_prefix: String,
    pub role: String,
    pub created_by: String,
    pub accepted_by: Option<String>,
    pub accepted_at: Option<DateTime<Utc>>,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub tenant_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AuditLogRow {
    pub id: String,
    pub tenant_id: String,
    pub actor_type: String,
    pub actor_id: String,
    pub action: String,
    pub resource_type: Option<String>,
    pub resource_id: Option<String>,
    pub source: String,
    pub metadata: Option<String>,
    pub created_at: DateTime<Utc>,
}
