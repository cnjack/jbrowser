use sqlx::{MySqlPool, Row};
use uuid::Uuid;

use super::models::{MemberRow, UserTenantRow};

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
