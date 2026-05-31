use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::db::repo;
use crate::server::auth::middleware::{authorize, require_admin};
use crate::server::error::AppError;
use crate::server::state::AppState;

pub async fn list_members_handler(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    authorize(&state, &headers, Some(tenant_id)).await?;
    let rows = repo::list_members(&state.pool, tenant_id)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    Ok(Json(json!({
        "members": rows.iter().map(|r| json!({
            "user_id": r.user_id,
            "email": r.email,
            "display_name": r.display_name,
            "role": r.role,
            "joined_at": r.joined_at
        })).collect::<Vec<_>>()
    })))
}

#[derive(Deserialize)]
pub struct UpdateRoleRequest {
    role: String,
}

pub async fn update_member_role_handler(
    State(state): State<AppState>,
    Path((tenant_id, target_user_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(req): Json<UpdateRoleRequest>,
) -> Result<Json<Value>, AppError> {
    let claims = require_admin(&state, &headers, tenant_id).await?;
    if req.role != "admin" && req.role != "member" {
        return Err(AppError::bad_request("role must be admin or member"));
    }
    let caller_id = Uuid::parse_str(&claims.sub).unwrap();
    if caller_id == target_user_id {
        return Err(AppError::bad_request("cannot change your own role"));
    }
    if req.role == "member" {
        let admin_count = repo::count_admins(&state.pool, tenant_id)
            .await
            .map_err(|e| AppError::internal(e.to_string()))?;
        let current_role = repo::get_member_role(&state.pool, tenant_id, target_user_id)
            .await
            .map_err(|e| AppError::internal(e.to_string()))?;
        if current_role.as_deref() == Some("admin") && admin_count <= 1 {
            return Err(AppError::bad_request("cannot demote the last admin"));
        }
    }
    repo::update_member_role(&state.pool, tenant_id, target_user_id, &req.role)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    Ok(Json(json!({"message": "role updated"})))
}

pub async fn remove_member(
    State(state): State<AppState>,
    Path((tenant_id, target_user_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<StatusCode, AppError> {
    let claims = require_admin(&state, &headers, tenant_id).await?;
    let caller_id = Uuid::parse_str(&claims.sub).unwrap();
    if caller_id == target_user_id {
        return Err(AppError::bad_request(
            "cannot remove yourself, use leave instead",
        ));
    }
    let removed = repo::remove_tenant_member(&state.pool, tenant_id, target_user_id)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    if removed {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::not_found("member"))
    }
}

pub async fn leave_tenant(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, AppError> {
    let claims = authorize(&state, &headers, Some(tenant_id)).await?;
    let user_id = Uuid::parse_str(&claims.sub).unwrap();
    let role = repo::get_member_role(&state.pool, tenant_id, user_id)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?
        .ok_or(AppError::not_found("membership"))?;
    if role == "admin" {
        let count = repo::count_admins(&state.pool, tenant_id)
            .await
            .map_err(|e| AppError::internal(e.to_string()))?;
        if count <= 1 {
            return Err(AppError::bad_request(
                "cannot leave: you are the last admin",
            ));
        }
    }
    repo::remove_tenant_member(&state.pool, tenant_id, user_id)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}
