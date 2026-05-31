use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::db::repo;
use crate::server::auth::crypto::issue_jwt;
use crate::server::auth::handlers::generate_slug;
use crate::server::auth::middleware::{authorize, require_admin};
use crate::server::error::AppError;
use crate::server::state::AppState;
use crate::server::types::{Tenant, TenantClaim, User};

#[derive(Deserialize)]
pub struct UpdateTenantRequest {
    name: String,
}

#[derive(Deserialize)]
pub struct CreateTenantRequest {
    name: String,
}

pub async fn create_tenant_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateTenantRequest>,
) -> Result<Json<Value>, AppError> {
    let name = req.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::bad_request("tenant name cannot be empty"));
    }
    let claims = authorize(&state, &headers, None).await?;
    let user_id = Uuid::parse_str(&claims.sub).unwrap();
    let tenant_id = Uuid::now_v7();
    let slug = generate_slug(&name);
    repo::create_tenant(&state.pool, tenant_id, &name, &slug)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    repo::add_tenant_member(&state.pool, tenant_id, user_id, "admin")
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
        .ok_or(AppError::unauthorized("user not found"))?;
    let user = User {
        id: user_id,
        email: user_row.email.clone(),
        display_name: user_row.display_name.clone().unwrap_or_default(),
        password_hash: user_row.password_hash.clone(),
        tenants: tenants_claim,
    };
    let access_token = issue_jwt(&state.config.jwt_secret, &user)?;
    let tenants_resp: Vec<Tenant> = tenant_rows
        .iter()
        .map(|t| Tenant {
            id: Uuid::parse_str(&t.id).unwrap(),
            name: t.name.clone(),
            slug: t.slug.clone(),
            role: t.role.clone(),
        })
        .collect();

    Ok(Json(json!({
        "access_token": access_token,
        "token_type": "Bearer",
        "tenant": {
            "id": tenant_id,
            "name": name,
            "slug": slug,
            "role": "admin"
        },
        "tenants": tenants_resp
    })))
}

pub async fn update_tenant_handler(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
    headers: HeaderMap,
    Json(req): Json<UpdateTenantRequest>,
) -> Result<Json<Value>, AppError> {
    require_admin(&state, &headers, tenant_id).await?;
    repo::update_tenant(&state.pool, tenant_id, &req.name)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    Ok(Json(json!({"message": "tenant updated"})))
}

pub async fn delete_tenant_handler(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, AppError> {
    require_admin(&state, &headers, tenant_id).await?;
    {
        let browsers = state.browsers.read().await;
        let browser_ids: Vec<Uuid> = browsers
            .values()
            .filter(|b| b.tenant_id == tenant_id)
            .map(|b| b.id)
            .collect();
        let agent_ids: Vec<Uuid> = browsers
            .values()
            .filter(|b| b.tenant_id == tenant_id)
            .map(|b| b.agent_id)
            .collect();
        drop(browsers);
        state
            .browsers
            .write()
            .await
            .retain(|_, b| b.tenant_id != tenant_id);
        state
            .agents
            .write()
            .await
            .retain(|_, a| a.tenant_id != tenant_id);
        for bid in &browser_ids {
            state.browser_preview.write().await.remove(bid);
            state.browser_last_frame.write().await.remove(bid);
            state.browser_events.write().await.remove(bid);
        }
        for aid in &agent_ids {
            state.agent_senders.write().await.remove(aid);
        }
    }
    repo::delete_tenant(&state.pool, tenant_id)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}
