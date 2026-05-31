use chrono::{DateTime, Utc};
use sqlx::{MySqlPool, Row};
use uuid::Uuid;

// ── Row structs ─────────────────────────────────────────────────────────

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

// ── Tenant ──────────────────────────────────────────────────────────────

pub async fn create_tenant(pool: &MySqlPool, id: Uuid, name: &str, slug: &str) -> sqlx::Result<()> {
    sqlx::query("INSERT INTO tenants (id, name, slug) VALUES (?, ?, ?)")
        .bind(id.to_string())
        .bind(name)
        .bind(slug)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_tenant(pool: &MySqlPool, id: Uuid) -> sqlx::Result<Option<TenantRow>> {
    let row = sqlx::query("SELECT id, name, slug FROM tenants WHERE id = ?")
        .bind(id.to_string())
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| TenantRow {
        id: r.get("id"),
        name: r.get("name"),
        slug: r.get("slug"),
    }))
}

pub async fn update_tenant(pool: &MySqlPool, id: Uuid, name: &str) -> sqlx::Result<bool> {
    let result = sqlx::query("UPDATE tenants SET name = ? WHERE id = ?")
        .bind(name)
        .bind(id.to_string())
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn delete_tenant(pool: &MySqlPool, id: Uuid) -> sqlx::Result<()> {
    let tid = id.to_string();
    sqlx::query("DELETE FROM audit_logs WHERE tenant_id = ?")
        .bind(&tid)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM invitations WHERE tenant_id = ?")
        .bind(&tid)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM tokens WHERE tenant_id = ?")
        .bind(&tid)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM browser_instances WHERE tenant_id = ?")
        .bind(&tid)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM agents WHERE tenant_id = ?")
        .bind(&tid)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM tenant_members WHERE tenant_id = ?")
        .bind(&tid)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM tenants WHERE id = ?")
        .bind(&tid)
        .execute(pool)
        .await?;
    Ok(())
}

// ── User ────────────────────────────────────────────────────────────────

pub async fn create_user(
    pool: &MySqlPool,
    id: Uuid,
    email: &str,
    password_hash: &str,
    display_name: &str,
) -> sqlx::Result<()> {
    sqlx::query("INSERT INTO users (id, email, password_hash, display_name) VALUES (?, ?, ?, ?)")
        .bind(id.to_string())
        .bind(email)
        .bind(password_hash)
        .bind(display_name)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_user_by_email(pool: &MySqlPool, email: &str) -> sqlx::Result<Option<UserRow>> {
    let row = sqlx::query(
        "SELECT id, email, password_hash, display_name, is_platform_admin FROM users WHERE email = ?",
    )
    .bind(email)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| UserRow {
        id: r.get("id"),
        email: r.get("email"),
        password_hash: r.get("password_hash"),
        display_name: r.get("display_name"),
        is_platform_admin: r.get::<bool, _>("is_platform_admin"),
    }))
}

pub async fn get_user_by_id(pool: &MySqlPool, id: Uuid) -> sqlx::Result<Option<UserRow>> {
    let row = sqlx::query(
        "SELECT id, email, password_hash, display_name, is_platform_admin FROM users WHERE id = ?",
    )
    .bind(id.to_string())
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| UserRow {
        id: r.get("id"),
        email: r.get("email"),
        password_hash: r.get("password_hash"),
        display_name: r.get("display_name"),
        is_platform_admin: r.get::<bool, _>("is_platform_admin"),
    }))
}

pub async fn update_user_password(
    pool: &MySqlPool,
    id: Uuid,
    password_hash: &str,
) -> sqlx::Result<bool> {
    let result = sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
        .bind(password_hash)
        .bind(id.to_string())
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

// ── Tenant Members ──────────────────────────────────────────────────────

pub async fn add_tenant_member(
    pool: &MySqlPool,
    tenant_id: Uuid,
    user_id: Uuid,
    role: &str,
) -> sqlx::Result<()> {
    sqlx::query("INSERT INTO tenant_members (tenant_id, user_id, role) VALUES (?, ?, ?)")
        .bind(tenant_id.to_string())
        .bind(user_id.to_string())
        .bind(role)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn remove_tenant_member(
    pool: &MySqlPool,
    tenant_id: Uuid,
    user_id: Uuid,
) -> sqlx::Result<bool> {
    let result = sqlx::query("DELETE FROM tenant_members WHERE tenant_id = ? AND user_id = ?")
        .bind(tenant_id.to_string())
        .bind(user_id.to_string())
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn update_member_role(
    pool: &MySqlPool,
    tenant_id: Uuid,
    user_id: Uuid,
    role: &str,
) -> sqlx::Result<bool> {
    let result =
        sqlx::query("UPDATE tenant_members SET role = ? WHERE tenant_id = ? AND user_id = ?")
            .bind(role)
            .bind(tenant_id.to_string())
            .bind(user_id.to_string())
            .execute(pool)
            .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn get_member_role(
    pool: &MySqlPool,
    tenant_id: Uuid,
    user_id: Uuid,
) -> sqlx::Result<Option<String>> {
    let row = sqlx::query("SELECT role FROM tenant_members WHERE tenant_id = ? AND user_id = ?")
        .bind(tenant_id.to_string())
        .bind(user_id.to_string())
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| r.get("role")))
}

pub async fn list_members(pool: &MySqlPool, tenant_id: Uuid) -> sqlx::Result<Vec<MemberRow>> {
    let rows = sqlx::query(
        "SELECT tm.user_id, tm.role, tm.created_at, u.email, u.display_name \
         FROM tenant_members tm JOIN users u ON tm.user_id = u.id \
         WHERE tm.tenant_id = ?",
    )
    .bind(tenant_id.to_string())
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| MemberRow {
            user_id: r.get("user_id"),
            email: r.get("email"),
            display_name: r.get("display_name"),
            role: r.get("role"),
            joined_at: r.get("created_at"),
        })
        .collect())
}

pub async fn get_user_tenants(pool: &MySqlPool, user_id: Uuid) -> sqlx::Result<Vec<UserTenantRow>> {
    let rows = sqlx::query(
        "SELECT t.id, t.name, t.slug, tm.role \
         FROM tenant_members tm JOIN tenants t ON tm.tenant_id = t.id \
         WHERE tm.user_id = ?",
    )
    .bind(user_id.to_string())
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| UserTenantRow {
            id: r.get("id"),
            name: r.get("name"),
            slug: r.get("slug"),
            role: r.get("role"),
        })
        .collect())
}

pub async fn count_admins(pool: &MySqlPool, tenant_id: Uuid) -> sqlx::Result<i64> {
    let row = sqlx::query(
        "SELECT COUNT(*) as count FROM tenant_members WHERE tenant_id = ? AND role = 'admin'",
    )
    .bind(tenant_id.to_string())
    .fetch_one(pool)
    .await?;
    Ok(row.get("count"))
}

// ── Tokens ──────────────────────────────────────────────────────────────

pub struct CreateToken<'a> {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub token_type: &'a str,
    pub name: Option<&'a str>,
    pub token_hash: &'a str,
    pub token_prefix: &'a str,
    pub created_by: Option<&'a str>,
}

pub async fn create_token(pool: &MySqlPool, t: &CreateToken<'_>) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO tokens (id, tenant_id, token_type, name, token_hash, token_prefix, created_by) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(t.id.to_string())
    .bind(t.tenant_id.to_string())
    .bind(t.token_type)
    .bind(t.name)
    .bind(t.token_hash)
    .bind(t.token_prefix)
    .bind(t.created_by)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_token_by_hash(
    pool: &MySqlPool,
    token_hash: &str,
) -> sqlx::Result<Option<TokenRow>> {
    let row = sqlx::query(
        "SELECT id, tenant_id, token_type, name, token_hash, token_prefix, created_by, revoked_at, created_at \
         FROM tokens WHERE token_hash = ? AND revoked_at IS NULL",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| TokenRow {
        id: r.get("id"),
        tenant_id: r.get("tenant_id"),
        token_type: r.get("token_type"),
        name: r.get("name"),
        token_hash: r.get("token_hash"),
        token_prefix: r.get("token_prefix"),
        created_by: r.get("created_by"),
        revoked_at: r.get("revoked_at"),
        created_at: r.get("created_at"),
    }))
}

pub async fn list_tokens(
    pool: &MySqlPool,
    tenant_id: Uuid,
    token_type: &str,
) -> sqlx::Result<Vec<TokenRow>> {
    let rows = sqlx::query(
        "SELECT id, tenant_id, token_type, name, token_hash, token_prefix, created_by, revoked_at, created_at \
         FROM tokens WHERE tenant_id = ? AND token_type = ?",
    )
    .bind(tenant_id.to_string())
    .bind(token_type)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| TokenRow {
            id: r.get("id"),
            tenant_id: r.get("tenant_id"),
            token_type: r.get("token_type"),
            name: r.get("name"),
            token_hash: r.get("token_hash"),
            token_prefix: r.get("token_prefix"),
            created_by: r.get("created_by"),
            revoked_at: r.get("revoked_at"),
            created_at: r.get("created_at"),
        })
        .collect())
}

pub async fn revoke_token(pool: &MySqlPool, tenant_id: Uuid, token_id: Uuid) -> sqlx::Result<bool> {
    let result = sqlx::query("UPDATE tokens SET revoked_at = NOW() WHERE id = ? AND tenant_id = ?")
        .bind(token_id.to_string())
        .bind(tenant_id.to_string())
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn delete_token(pool: &MySqlPool, tenant_id: Uuid, token_id: Uuid) -> sqlx::Result<bool> {
    let result = sqlx::query("DELETE FROM tokens WHERE id = ? AND tenant_id = ?")
        .bind(token_id.to_string())
        .bind(tenant_id.to_string())
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

// ── Agents ──────────────────────────────────────────────────────────────

pub async fn create_agent(
    pool: &MySqlPool,
    id: Uuid,
    tenant_id: Uuid,
    browser_instance_id: Uuid,
    name: &str,
    runtime_token_hash: &str,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO agents (id, tenant_id, browser_instance_id, name, runtime_token_hash) \
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(id.to_string())
    .bind(tenant_id.to_string())
    .bind(browser_instance_id.to_string())
    .bind(name)
    .bind(runtime_token_hash)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_agent(
    pool: &MySqlPool,
    tenant_id: Uuid,
    agent_id: Uuid,
) -> sqlx::Result<Option<AgentRow>> {
    let row = sqlx::query(
        "SELECT id, tenant_id, browser_instance_id, name, runtime_token_hash, status, last_heartbeat_at \
         FROM agents WHERE id = ? AND tenant_id = ?",
    )
    .bind(agent_id.to_string())
    .bind(tenant_id.to_string())
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| AgentRow {
        id: r.get("id"),
        tenant_id: r.get("tenant_id"),
        browser_instance_id: r.get("browser_instance_id"),
        name: r.get("name"),
        runtime_token_hash: r.get("runtime_token_hash"),
        status: r.get("status"),
        last_heartbeat_at: r.get("last_heartbeat_at"),
    }))
}

pub async fn list_agents_by_tenant(
    pool: &MySqlPool,
    tenant_id: Uuid,
) -> sqlx::Result<Vec<AgentRow>> {
    let rows = sqlx::query(
        "SELECT id, tenant_id, browser_instance_id, name, runtime_token_hash, status, last_heartbeat_at \
         FROM agents WHERE tenant_id = ?",
    )
    .bind(tenant_id.to_string())
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| AgentRow {
            id: r.get("id"),
            tenant_id: r.get("tenant_id"),
            browser_instance_id: r.get("browser_instance_id"),
            name: r.get("name"),
            runtime_token_hash: r.get("runtime_token_hash"),
            status: r.get("status"),
            last_heartbeat_at: r.get("last_heartbeat_at"),
        })
        .collect())
}

pub async fn delete_agent(pool: &MySqlPool, tenant_id: Uuid, agent_id: Uuid) -> sqlx::Result<bool> {
    let result = sqlx::query("DELETE FROM agents WHERE id = ? AND tenant_id = ?")
        .bind(agent_id.to_string())
        .bind(tenant_id.to_string())
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn update_agent_status(
    pool: &MySqlPool,
    agent_id: Uuid,
    status: &str,
) -> sqlx::Result<bool> {
    let result =
        sqlx::query("UPDATE agents SET status = ?, last_heartbeat_at = NOW() WHERE id = ?")
            .bind(status)
            .bind(agent_id.to_string())
            .execute(pool)
            .await?;
    Ok(result.rows_affected() > 0)
}

/// Replace the runtime_token_hash on an existing agent (used on re-registration
/// so the agent keeps its stable id but gets a fresh token each boot).
pub async fn update_agent_runtime_token(
    pool: &MySqlPool,
    agent_id: Uuid,
    new_token_hash: &str,
) -> sqlx::Result<bool> {
    let result =
        sqlx::query("UPDATE agents SET runtime_token_hash = ? WHERE id = ?")
            .bind(new_token_hash)
            .bind(agent_id.to_string())
            .execute(pool)
            .await?;
    Ok(result.rows_affected() > 0)
}

/// Upsert the runtime token row (token name = "runtime-{agent_id}").
/// Deletes the old row (if any) and inserts a fresh one with the new hash.
pub async fn replace_agent_runtime_token(
    pool: &MySqlPool,
    agent_id: Uuid,
    tenant_id: Uuid,
    new_token_hash: &str,
    new_token_prefix: &str,
) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM tokens WHERE name = ? AND tenant_id = ?")
        .bind(format!("runtime-{agent_id}"))
        .bind(tenant_id.to_string())
        .execute(pool)
        .await?;
    sqlx::query(
        "INSERT INTO tokens (id, tenant_id, token_type, name, token_hash, token_prefix) \
         VALUES (?, ?, 'agent_runtime', ?, ?, ?)",
    )
    .bind(Uuid::now_v7().to_string())
    .bind(tenant_id.to_string())
    .bind(format!("runtime-{agent_id}"))
    .bind(new_token_hash)
    .bind(new_token_prefix)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_agent_by_runtime_token(
    pool: &MySqlPool,
    runtime_token_hash: &str,
) -> sqlx::Result<Option<AgentRow>> {
    let row = sqlx::query(
        "SELECT id, tenant_id, browser_instance_id, name, runtime_token_hash, status, last_heartbeat_at \
         FROM agents WHERE runtime_token_hash = ?",
    )
    .bind(runtime_token_hash)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| AgentRow {
        id: r.get("id"),
        tenant_id: r.get("tenant_id"),
        browser_instance_id: r.get("browser_instance_id"),
        name: r.get("name"),
        runtime_token_hash: r.get("runtime_token_hash"),
        status: r.get("status"),
        last_heartbeat_at: r.get("last_heartbeat_at"),
    }))
}

// ── Browser Instances ───────────────────────────────────────────────────

pub async fn create_browser_instance(
    pool: &MySqlPool,
    id: Uuid,
    tenant_id: Uuid,
    agent_id: Uuid,
    browser_type: &str,
    browser_version: &str,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO browser_instances (id, tenant_id, agent_id, browser_type, browser_version) \
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(id.to_string())
    .bind(tenant_id.to_string())
    .bind(agent_id.to_string())
    .bind(browser_type)
    .bind(browser_version)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_browser_instance(
    pool: &MySqlPool,
    tenant_id: Uuid,
    browser_id: Uuid,
) -> sqlx::Result<Option<BrowserRow>> {
    let row = sqlx::query(
        "SELECT id, tenant_id, agent_id, browser_type, browser_version, status, \
         active_tab_id, tabs_snapshot, viewport_width, viewport_height \
         FROM browser_instances WHERE id = ? AND tenant_id = ?",
    )
    .bind(browser_id.to_string())
    .bind(tenant_id.to_string())
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| BrowserRow {
        id: r.get("id"),
        tenant_id: r.get("tenant_id"),
        agent_id: r.get("agent_id"),
        browser_type: r.get("browser_type"),
        browser_version: r.get("browser_version"),
        status: r.get("status"),
        active_tab_id: r.get("active_tab_id"),
        tabs_snapshot: r
            .get::<Option<serde_json::Value>, _>("tabs_snapshot")
            .map(|v| v.to_string()),
        viewport_width: r.get("viewport_width"),
        viewport_height: r.get("viewport_height"),
    }))
}

pub async fn list_browsers_by_tenant(
    pool: &MySqlPool,
    tenant_id: Uuid,
) -> sqlx::Result<Vec<BrowserRow>> {
    let rows = sqlx::query(
        "SELECT id, tenant_id, agent_id, browser_type, browser_version, status, \
         active_tab_id, tabs_snapshot, viewport_width, viewport_height \
         FROM browser_instances WHERE tenant_id = ?",
    )
    .bind(tenant_id.to_string())
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| BrowserRow {
            id: r.get("id"),
            tenant_id: r.get("tenant_id"),
            agent_id: r.get("agent_id"),
            browser_type: r.get("browser_type"),
            browser_version: r.get("browser_version"),
            status: r.get("status"),
            active_tab_id: r.get("active_tab_id"),
            tabs_snapshot: r
                .get::<Option<serde_json::Value>, _>("tabs_snapshot")
                .map(|v| v.to_string()),
            viewport_width: r.get("viewport_width"),
            viewport_height: r.get("viewport_height"),
        })
        .collect())
}

pub async fn delete_browser_instance(
    pool: &MySqlPool,
    tenant_id: Uuid,
    browser_id: Uuid,
) -> sqlx::Result<bool> {
    let result = sqlx::query("DELETE FROM browser_instances WHERE id = ? AND tenant_id = ?")
        .bind(browser_id.to_string())
        .bind(tenant_id.to_string())
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn update_browser_instance(
    pool: &MySqlPool,
    browser_id: Uuid,
    status: &str,
    active_tab_id: Option<&str>,
    tabs_snapshot: Option<&str>,
) -> sqlx::Result<bool> {
    let result = sqlx::query(
        "UPDATE browser_instances SET status = ?, active_tab_id = ?, tabs_snapshot = ? WHERE id = ?",
    )
    .bind(status)
    .bind(active_tab_id)
    .bind(tabs_snapshot)
    .bind(browser_id.to_string())
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

// ── Invitations ─────────────────────────────────────────────────────────

pub struct CreateInvitation<'a> {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub token_hash: &'a str,
    pub token_prefix: &'a str,
    pub role: &'a str,
    pub created_by: Uuid,
    pub expires_at: DateTime<Utc>,
}

pub async fn create_invitation(pool: &MySqlPool, inv: &CreateInvitation<'_>) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO invitations (id, tenant_id, token_hash, token_prefix, role, created_by, expires_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(inv.id.to_string())
    .bind(inv.tenant_id.to_string())
    .bind(inv.token_hash)
    .bind(inv.token_prefix)
    .bind(inv.role)
    .bind(inv.created_by.to_string())
    .bind(inv.expires_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_invitation_by_token_hash(
    pool: &MySqlPool,
    token_hash: &str,
) -> sqlx::Result<Option<InvitationRow>> {
    let row = sqlx::query(
        "SELECT i.id, i.tenant_id, i.token_hash, i.token_prefix, i.role, i.created_by, \
         i.accepted_by, i.accepted_at, i.expires_at, i.created_at, t.name as tenant_name \
         FROM invitations i JOIN tenants t ON i.tenant_id = t.id \
         WHERE i.token_hash = ? AND i.accepted_at IS NULL AND i.expires_at > NOW()",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| InvitationRow {
        id: r.get("id"),
        tenant_id: r.get("tenant_id"),
        token_hash: r.get("token_hash"),
        token_prefix: r.get("token_prefix"),
        role: r.get("role"),
        created_by: r.get("created_by"),
        accepted_by: r.get("accepted_by"),
        accepted_at: r.get("accepted_at"),
        expires_at: r.get("expires_at"),
        created_at: r.get("created_at"),
        tenant_name: r.get("tenant_name"),
    }))
}

pub async fn list_invitations_by_tenant(
    pool: &MySqlPool,
    tenant_id: Uuid,
) -> sqlx::Result<Vec<InvitationRow>> {
    let rows = sqlx::query(
        "SELECT id, tenant_id, token_hash, token_prefix, role, created_by, \
         accepted_by, accepted_at, expires_at, created_at \
         FROM invitations WHERE tenant_id = ? ORDER BY created_at DESC",
    )
    .bind(tenant_id.to_string())
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| InvitationRow {
            id: r.get("id"),
            tenant_id: r.get("tenant_id"),
            token_hash: r.get("token_hash"),
            token_prefix: r.get("token_prefix"),
            role: r.get("role"),
            created_by: r.get("created_by"),
            accepted_by: r.get("accepted_by"),
            accepted_at: r.get("accepted_at"),
            expires_at: r.get("expires_at"),
            created_at: r.get("created_at"),
            tenant_name: None,
        })
        .collect())
}

pub async fn accept_invitation(
    pool: &MySqlPool,
    invitation_id: Uuid,
    accepted_by: Uuid,
) -> sqlx::Result<bool> {
    let result = sqlx::query(
        "UPDATE invitations SET accepted_by = ?, accepted_at = NOW() WHERE id = ? AND accepted_at IS NULL",
    )
    .bind(accepted_by.to_string())
    .bind(invitation_id.to_string())
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn delete_invitation(
    pool: &MySqlPool,
    tenant_id: Uuid,
    invitation_id: Uuid,
) -> sqlx::Result<bool> {
    let result = sqlx::query("DELETE FROM invitations WHERE id = ? AND tenant_id = ?")
        .bind(invitation_id.to_string())
        .bind(tenant_id.to_string())
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

// ── Audit Logs ──────────────────────────────────────────────────────────

pub struct CreateAuditLog<'a> {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub actor_type: &'a str,
    pub actor_id: &'a str,
    pub action: &'a str,
    pub source: &'a str,
    pub resource_type: Option<&'a str>,
    pub resource_id: Option<&'a str>,
    pub browser_instance_id: Option<Uuid>,
    pub tab_id: Option<&'a str>,
    pub metadata: Option<&'a str>,
}

pub async fn create_audit_log(pool: &MySqlPool, log: &CreateAuditLog<'_>) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO audit_logs (id, tenant_id, actor_type, actor_id, action, source, \
         resource_type, resource_id, browser_instance_id, tab_id, metadata) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(log.id.to_string())
    .bind(log.tenant_id.to_string())
    .bind(log.actor_type)
    .bind(log.actor_id)
    .bind(log.action)
    .bind(log.source)
    .bind(log.resource_type)
    .bind(log.resource_id)
    .bind(log.browser_instance_id.map(|u| u.to_string()))
    .bind(log.tab_id)
    .bind(log.metadata)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_audit_logs(
    pool: &MySqlPool,
    tenant_id: Uuid,
    limit: i64,
) -> sqlx::Result<Vec<AuditLogRow>> {
    let rows = sqlx::query(
        "SELECT id, tenant_id, actor_type, actor_id, action, resource_type, resource_id, \
         source, metadata, created_at \
         FROM audit_logs WHERE tenant_id = ? ORDER BY created_at DESC LIMIT ?",
    )
    .bind(tenant_id.to_string())
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| AuditLogRow {
            id: r.get("id"),
            tenant_id: r.get("tenant_id"),
            actor_type: r.get("actor_type"),
            actor_id: r.get("actor_id"),
            action: r.get("action"),
            resource_type: r.get("resource_type"),
            resource_id: r.get("resource_id"),
            source: r.get("source"),
            metadata: r
                .get::<Option<serde_json::Value>, _>("metadata")
                .map(|v| v.to_string()),
            created_at: r.get("created_at"),
        })
        .collect())
}
