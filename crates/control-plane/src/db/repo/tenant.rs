use sqlx::{MySqlPool, Row};
use uuid::Uuid;

use super::models::TenantRow;

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
