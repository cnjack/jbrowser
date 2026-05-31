use sqlx::{MySqlPool, Row};
use uuid::Uuid;

use super::models::AuditLogRow;

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
