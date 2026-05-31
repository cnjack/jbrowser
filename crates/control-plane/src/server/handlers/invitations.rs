use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::{Duration as ChronoDuration, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::db::repo;
use crate::server::auth::crypto::{generate_token, hash_token, issue_jwt};
use crate::server::auth::middleware::{authorize, require_admin};
use crate::server::error::AppError;
use crate::server::state::AppState;
use crate::server::types::{TenantClaim, User};

#[derive(Deserialize)]
pub struct CreateInvitationRequest {
    role: Option<String>,
}

pub async fn create_invitation(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
    headers: HeaderMap,
    Json(req): Json<CreateInvitationRequest>,
) -> Result<Json<Value>, AppError> {
    let claims = require_admin(&state, &headers, tenant_id).await?;
    let role = req.role.unwrap_or_else(|| "member".to_string());
    if role != "admin" && role != "member" {
        return Err(AppError::bad_request("role must be admin or member"));
    }
    let id = Uuid::now_v7();
    let raw_token = generate_token("jbr_inv");
    let token_hash = hash_token(&raw_token);
    let prefix: String = raw_token.chars().take(8).collect();
    let expires_at = Utc::now() + ChronoDuration::days(7);
    let user_id = Uuid::parse_str(&claims.sub).unwrap();
    repo::create_invitation(
        &state.pool,
        &repo::CreateInvitation {
            id,
            tenant_id,
            token_hash: &token_hash,
            token_prefix: &prefix,
            role: &role,
            created_by: user_id,
            expires_at,
        },
    )
    .await
    .map_err(|e| AppError::internal(e.to_string()))?;
    let invite_link = format!("{}/invite/{}", state.config.public_base_url, raw_token);
    Ok(Json(json!({
        "id": id,
        "invite_link": invite_link,
        "token": raw_token,
        "role": role,
        "created_by": claims.sub,
        "expires_at": expires_at
    })))
}

pub async fn list_invitations(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    require_admin(&state, &headers, tenant_id).await?;
    let rows = repo::list_invitations_by_tenant(&state.pool, tenant_id)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    Ok(Json(json!({
        "data": rows.iter().map(|r| json!({
            "id": r.id,
            "role": r.role,
            "token_prefix": r.token_prefix,
            "created_by": r.created_by,
            "accepted_by": r.accepted_by,
            "accepted_at": r.accepted_at,
            "expires_at": r.expires_at,
            "created_at": r.created_at
        })).collect::<Vec<_>>()
    })))
}

pub async fn delete_invitation_handler(
    State(state): State<AppState>,
    Path((tenant_id, invitation_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<StatusCode, AppError> {
    require_admin(&state, &headers, tenant_id).await?;
    let deleted = repo::delete_invitation(&state.pool, tenant_id, invitation_id)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::not_found("invitation"))
    }
}

pub async fn validate_invitation(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<Json<Value>, AppError> {
    let token_hash = hash_token(&token);
    let inv = repo::get_invitation_by_token_hash(&state.pool, &token_hash)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    match inv {
        Some(row) => Ok(Json(json!({
            "valid": true,
            "tenant_name": row.tenant_name,
            "role": row.role
        }))),
        None => Ok(Json(json!({ "valid": false }))),
    }
}

pub async fn accept_invitation(
    State(state): State<AppState>,
    Path(token): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    let claims = authorize(&state, &headers, None).await?;
    let user_id = Uuid::parse_str(&claims.sub).unwrap();
    let token_hash = hash_token(&token);
    let inv = repo::get_invitation_by_token_hash(&state.pool, &token_hash)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?
        .ok_or(AppError::bad_request("invalid or expired invitation"))?;
    let tenant_id = Uuid::parse_str(&inv.tenant_id).unwrap();
    let inv_id = Uuid::parse_str(&inv.id).unwrap();
    if repo::get_member_role(&state.pool, tenant_id, user_id)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?
        .is_some()
    {
        return Err(AppError::bad_request("already a member of this tenant"));
    }
    repo::add_tenant_member(&state.pool, tenant_id, user_id, &inv.role)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    repo::accept_invitation(&state.pool, inv_id, user_id)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    let tenant_rows = repo::get_user_tenants(&state.pool, user_id)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    let tenants_claim: Vec<TenantClaim> = tenant_rows
        .iter()
        .map(|t| TenantClaim {
            id: Uuid::parse_str(&t.id).unwrap(),
            role: t.role.clone(),
        })
        .collect();
    let user_row = repo::get_user_by_id(&state.pool, user_id)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?
        .unwrap();
    let user = User {
        id: user_id,
        email: user_row.email,
        display_name: user_row.display_name.unwrap_or_default(),
        password_hash: user_row.password_hash,
        tenants: tenants_claim,
    };
    let access_token = issue_jwt(&state.config.jwt_secret, &user)?;
    Ok(Json(json!({ "access_token": access_token })))
}
