use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Json,
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::db::repo;
use crate::server::auth::middleware::require_admin;
use crate::server::error::AppError;
use crate::server::state::AppState;

pub async fn list_audit_logs(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    require_admin(&state, &headers, tenant_id).await?;
    let rows = repo::list_audit_logs(&state.pool, tenant_id, 200)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    let data: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.id,
                "tenant_id": r.tenant_id,
                "actor_type": r.actor_type,
                "actor_id": r.actor_id,
                "action": r.action,
                "resource_type": r.resource_type,
                "resource_id": r.resource_id,
                "source": r.source,
                "metadata": r.metadata,
                "created_at": r.created_at
            })
        })
        .collect();
    Ok(Json(json!({ "data": data })))
}
