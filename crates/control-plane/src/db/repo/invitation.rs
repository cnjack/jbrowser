use chrono::{DateTime, Utc};
use sqlx::{MySqlPool, Row};
use uuid::Uuid;

use super::models::InvitationRow;

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
