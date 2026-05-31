use sqlx::{MySqlPool, Row};
use uuid::Uuid;

use super::models::TokenRow;

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
