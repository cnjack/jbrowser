use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::db::repo;
use crate::server::auth::crypto::{generate_token, hash_token};
use crate::server::auth::middleware::require_admin;
use crate::server::error::AppError;
use crate::server::state::AppState;

pub async fn list_cdp_tokens(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    crate::server::auth::middleware::authorize(&state, &headers, Some(tenant_id)).await?;
    let rows = repo::list_tokens(&state.pool, tenant_id, "tenant_cdp_access")
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    let data: Vec<_> = rows.iter().map(token_row_to_response).collect();
    Ok(Json(json!({ "data": data })))
}

pub async fn list_agent_registration_tokens(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    crate::server::auth::middleware::authorize(&state, &headers, Some(tenant_id)).await?;
    let rows = repo::list_tokens(&state.pool, tenant_id, "agent_registration")
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    let data: Vec<_> = rows.iter().map(token_row_to_response).collect();
    Ok(Json(json!({ "data": data })))
}

#[derive(Debug, Deserialize)]
pub struct CreateTokenRequest {
    name: Option<String>,
}

pub async fn create_cdp_token(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
    headers: HeaderMap,
    body: Option<Json<CreateTokenRequest>>,
) -> Result<Json<Value>, AppError> {
    let name = body.and_then(|Json(req)| req.name);
    create_token_impl(
        state,
        tenant_id,
        headers,
        "tenant_cdp_access",
        "jbr_cdp",
        name,
    )
    .await
}

pub async fn create_agent_registration_token(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
    headers: HeaderMap,
    body: Option<Json<CreateTokenRequest>>,
) -> Result<Json<Value>, AppError> {
    let name = body.and_then(|Json(req)| req.name);
    create_token_impl(
        state,
        tenant_id,
        headers,
        "agent_registration",
        "jbr_reg",
        name,
    )
    .await
}

async fn create_token_impl(
    state: AppState,
    tenant_id: Uuid,
    headers: HeaderMap,
    token_type: &str,
    prefix: &str,
    name: Option<String>,
) -> Result<Json<Value>, AppError> {
    let claims = require_admin(&state, &headers, tenant_id).await?;
    let raw = generate_token(prefix);
    let id = Uuid::now_v7();
    let thash = hash_token(&raw);
    let tprefix: String = raw.chars().take(12).collect();

    repo::create_token(
        &state.pool,
        &repo::CreateToken {
            id,
            tenant_id,
            token_type,
            name: name.as_deref(),
            token_hash: &thash,
            token_prefix: &tprefix,
            created_by: Some(&claims.sub),
        },
    )
    .await
    .map_err(|e| AppError::internal(e.to_string()))?;

    let _ = repo::create_audit_log(
        &state.pool,
        &repo::CreateAuditLog {
            id: Uuid::now_v7(),
            tenant_id,
            actor_type: "user",
            actor_id: &claims.sub,
            action: "token.created",
            source: "web_ui",
            resource_type: None,
            resource_id: None,
            browser_instance_id: None,
            tab_id: None,
            metadata: None,
        },
    )
    .await;

    let response = json!({
        "data": {
            "id": id,
            "tenant_id": tenant_id,
            "token_type": token_type,
            "name": name,
            "token_prefix": tprefix,
            "created_by": claims.sub,
            "revoked_at": null,
            "created_at": Utc::now()
        },
        "token": raw
    });
    Ok(Json(response))
}

pub async fn revoke_token_handler(
    State(state): State<AppState>,
    Path((tenant_id, token_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    let claims = require_admin(&state, &headers, tenant_id).await?;
    let revoked = repo::revoke_token(&state.pool, tenant_id, token_id)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    if !revoked {
        return Err(AppError::not_found("token"));
    }
    let _ = repo::create_audit_log(
        &state.pool,
        &repo::CreateAuditLog {
            id: Uuid::now_v7(),
            tenant_id,
            actor_type: "user",
            actor_id: &claims.sub,
            action: "token.revoked",
            source: "web_ui",
            resource_type: None,
            resource_id: None,
            browser_instance_id: None,
            tab_id: None,
            metadata: None,
        },
    )
    .await;
    Ok(Json(json!({ "data": { "id": token_id, "revoked": true } })))
}

pub async fn rotate_token(
    State(state): State<AppState>,
    Path((tenant_id, token_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    let claims = require_admin(&state, &headers, tenant_id).await?;
    let _ = repo::revoke_token(&state.pool, tenant_id, token_id)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    let raw = generate_token("jbr_cdp");
    let new_id = Uuid::now_v7();
    let thash = hash_token(&raw);
    let tprefix: String = raw.chars().take(12).collect();
    repo::create_token(
        &state.pool,
        &repo::CreateToken {
            id: new_id,
            tenant_id,
            token_type: "tenant_cdp_access",
            name: None,
            token_hash: &thash,
            token_prefix: &tprefix,
            created_by: Some(&claims.sub),
        },
    )
    .await
    .map_err(|e| AppError::internal(e.to_string()))?;

    let _ = repo::create_audit_log(
        &state.pool,
        &repo::CreateAuditLog {
            id: Uuid::now_v7(),
            tenant_id,
            actor_type: "user",
            actor_id: &claims.sub,
            action: "token.rotated",
            source: "web_ui",
            resource_type: None,
            resource_id: None,
            browser_instance_id: None,
            tab_id: None,
            metadata: None,
        },
    )
    .await;

    Ok(Json(json!({
        "data": {
            "id": new_id,
            "tenant_id": tenant_id,
            "token_type": "tenant_cdp_access",
            "token_prefix": tprefix,
            "created_by": claims.sub,
            "revoked_at": null,
            "created_at": Utc::now()
        },
        "token": raw
    })))
}

fn token_row_to_response(row: &repo::TokenRow) -> Value {
    json!({
        "id": row.id,
        "tenant_id": row.tenant_id,
        "token_type": row.token_type,
        "name": row.name,
        "token_prefix": row.token_prefix,
        "created_by": row.created_by,
        "revoked_at": row.revoked_at,
        "created_at": row.created_at
    })
}
