use sqlx::{MySqlPool, Row};
use uuid::Uuid;

use super::models::BrowserRow;

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

fn browser_row_from_row(r: sqlx::mysql::MySqlRow) -> BrowserRow {
    BrowserRow {
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
        user_agent: r.get("user_agent"),
        device_scale_factor: r.get("device_scale_factor"),
        timezone: r.get("timezone"),
        locale: r.get("locale"),
        stealth_level: r.get("stealth_level"),
    }
}

const BROWSER_SELECT_COLS: &str = "id, tenant_id, agent_id, browser_type, browser_version, \
    status, active_tab_id, tabs_snapshot, viewport_width, viewport_height, \
    user_agent, device_scale_factor, timezone, locale, stealth_level";

pub async fn get_browser_instance(
    pool: &MySqlPool,
    tenant_id: Uuid,
    browser_id: Uuid,
) -> sqlx::Result<Option<BrowserRow>> {
    let row = sqlx::query(&format!(
        "SELECT {BROWSER_SELECT_COLS} FROM browser_instances WHERE id = ? AND tenant_id = ?"
    ))
    .bind(browser_id.to_string())
    .bind(tenant_id.to_string())
    .fetch_optional(pool)
    .await?;
    Ok(row.map(browser_row_from_row))
}

pub async fn list_browsers_by_tenant(
    pool: &MySqlPool,
    tenant_id: Uuid,
) -> sqlx::Result<Vec<BrowserRow>> {
    let rows = sqlx::query(&format!(
        "SELECT {BROWSER_SELECT_COLS} FROM browser_instances WHERE tenant_id = ?"
    ))
    .bind(tenant_id.to_string())
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(browser_row_from_row).collect())
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

/// Parameters for updating browser config fields.
pub struct UpdateBrowserConfig<'a> {
    pub tenant_id: Uuid,
    pub browser_id: Uuid,
    pub user_agent: Option<&'a str>,
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub device_scale_factor: f64,
    pub timezone: Option<&'a str>,
    pub locale: Option<&'a str>,
    pub stealth_level: &'a str,
}

/// Update browser config fields (fingerprint + stealth). Scoped by tenant_id.
pub async fn update_browser_config(
    pool: &MySqlPool,
    params: &UpdateBrowserConfig<'_>,
) -> sqlx::Result<bool> {
    let result = sqlx::query(
        "UPDATE browser_instances SET \
         user_agent = ?, viewport_width = ?, viewport_height = ?, \
         device_scale_factor = ?, timezone = ?, locale = ?, stealth_level = ? \
         WHERE id = ? AND tenant_id = ?",
    )
    .bind(params.user_agent)
    .bind(params.viewport_width)
    .bind(params.viewport_height)
    .bind(params.device_scale_factor)
    .bind(params.timezone)
    .bind(params.locale)
    .bind(params.stealth_level)
    .bind(params.browser_id.to_string())
    .bind(params.tenant_id.to_string())
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}
