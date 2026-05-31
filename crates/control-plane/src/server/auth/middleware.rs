use axum::http::HeaderMap;
use uuid::Uuid;

use super::crypto::decode_jwt;
use crate::server::error::AppError;
use crate::server::state::AppState;
use crate::server::types::JwtClaims;

pub async fn authorize(
    state: &AppState,
    headers: &HeaderMap,
    tenant_id: Option<Uuid>,
) -> Result<JwtClaims, AppError> {
    let token =
        bearer_token(headers).ok_or_else(|| AppError::unauthorized("missing bearer token"))?;
    let claims = decode_jwt(&state.config.jwt_secret, token)?;
    if let Some(tenant_id) = tenant_id {
        if !claims.tenants.iter().any(|tenant| tenant.id == tenant_id) {
            return Err(AppError::forbidden("token is not a member of this tenant"));
        }
    }
    Ok(claims)
}

pub async fn require_admin(
    state: &AppState,
    headers: &HeaderMap,
    tenant_id: Uuid,
) -> Result<JwtClaims, AppError> {
    let claims = authorize(state, headers, Some(tenant_id)).await?;
    let is_admin = claims
        .tenants
        .iter()
        .any(|t| t.id == tenant_id && t.role == "admin");
    if !is_admin {
        return Err(AppError::forbidden("admin role required"));
    }
    Ok(claims)
}

pub fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}
