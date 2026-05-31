use sqlx::{MySqlPool, Row};
use uuid::Uuid;

use super::models::AgentRow;

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

pub async fn update_agent_runtime_token(
    pool: &MySqlPool,
    agent_id: Uuid,
    new_token_hash: &str,
) -> sqlx::Result<bool> {
    let result = sqlx::query("UPDATE agents SET runtime_token_hash = ? WHERE id = ?")
        .bind(new_token_hash)
        .bind(agent_id.to_string())
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

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
