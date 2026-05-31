use std::collections::HashMap;

use axum::{
    extract::{Query, State},
    http::HeaderMap,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::db::repo;
use crate::server::auth::crypto::{hash_password, hash_token, issue_jwt, verify_password};
use crate::server::auth::middleware::authorize;
use crate::server::error::AppError;
use crate::server::state::AppState;
use crate::server::types::{Tenant, TenantClaim, User, UserResponse};

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Debug, serde::Serialize)]
pub struct LoginResponse {
    access_token: String,
    token_type: &'static str,
    user: UserResponse,
    tenants: Vec<Tenant>,
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AppError> {
    let user_row = repo::get_user_by_email(&state.pool, &req.email)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?
        .ok_or(AppError::unauthorized("invalid credentials"))?;

    verify_password(&req.password, &user_row.password_hash)?;

    let user_id = Uuid::parse_str(&user_row.id).unwrap();
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

    let user = User {
        id: user_id,
        email: user_row.email,
        display_name: user_row.display_name.unwrap_or_default(),
        password_hash: user_row.password_hash,
        tenants: tenants_claim.clone(),
    };
    let access_token = issue_jwt(&state.config.jwt_secret, &user)?;
    let tenants: Vec<Tenant> = tenant_rows
        .iter()
        .map(|t| Tenant {
            id: Uuid::parse_str(&t.id).unwrap(),
            name: t.name.clone(),
            slug: t.slug.clone(),
            role: t.role.clone(),
        })
        .collect();

    if let Some(tc) = tenants_claim.first() {
        let _ = repo::create_audit_log(
            &state.pool,
            &repo::CreateAuditLog {
                id: Uuid::now_v7(),
                tenant_id: tc.id,
                actor_type: "user",
                actor_id: &user_id.to_string(),
                action: "user.login",
                source: "web_ui",
                resource_type: None,
                resource_id: None,
                browser_instance_id: None,
                tab_id: None,
                metadata: None,
            },
        )
        .await;
    }

    Ok(Json(LoginResponse {
        access_token,
        token_type: "Bearer",
        user: UserResponse {
            id: user_id,
            email: user.email,
            display_name: user.display_name,
        },
        tenants,
    }))
}

#[derive(Deserialize)]
pub struct SignupRequest {
    email: String,
    password: String,
    display_name: String,
}

pub async fn signup(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
    Json(req): Json<SignupRequest>,
) -> Result<Json<LoginResponse>, AppError> {
    if req.password.len() < 8 {
        return Err(AppError::bad_request(
            "password must be at least 8 characters",
        ));
    }
    if repo::get_user_by_email(&state.pool, &req.email)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?
        .is_some()
    {
        return Err(AppError::bad_request("email already registered"));
    }

    let user_id = Uuid::now_v7();
    let pw_hash = hash_password(&req.password).map_err(|e| AppError::internal(e.to_string()))?;
    repo::create_user(
        &state.pool,
        user_id,
        &req.email,
        &pw_hash,
        &req.display_name,
    )
    .await
    .map_err(|e| AppError::internal(e.to_string()))?;

    let invite_token = params.get("invite_token");
    let mut tenants_claim = Vec::new();
    let mut tenants_resp = Vec::new();

    if let Some(raw_token) = invite_token {
        let token_hash = hash_token(raw_token);
        if let Some(inv) = repo::get_invitation_by_token_hash(&state.pool, &token_hash)
            .await
            .map_err(|e| AppError::internal(e.to_string()))?
        {
            let tenant_id = Uuid::parse_str(&inv.tenant_id).unwrap();
            repo::add_tenant_member(&state.pool, tenant_id, user_id, &inv.role)
                .await
                .map_err(|e| AppError::internal(e.to_string()))?;
            let inv_id = Uuid::parse_str(&inv.id).unwrap();
            repo::accept_invitation(&state.pool, inv_id, user_id)
                .await
                .map_err(|e| AppError::internal(e.to_string()))?;
            if let Some(t) = repo::get_tenant(&state.pool, tenant_id)
                .await
                .map_err(|e| AppError::internal(e.to_string()))?
            {
                tenants_claim.push(TenantClaim {
                    id: tenant_id,
                    role: inv.role.clone(),
                });
                tenants_resp.push(Tenant {
                    id: tenant_id,
                    name: t.name,
                    slug: t.slug,
                    role: inv.role.clone(),
                });
            }
        } else {
            return Err(AppError::bad_request("invalid or expired invitation"));
        }
    } else {
        let tenant_id = Uuid::now_v7();
        let slug = generate_slug(&req.display_name);
        let tenant_name = format!("{}'s Workspace", req.display_name);
        repo::create_tenant(&state.pool, tenant_id, &tenant_name, &slug)
            .await
            .map_err(|e| AppError::internal(e.to_string()))?;
        repo::add_tenant_member(&state.pool, tenant_id, user_id, "admin")
            .await
            .map_err(|e| AppError::internal(e.to_string()))?;
        tenants_claim.push(TenantClaim {
            id: tenant_id,
            role: "admin".to_string(),
        });
        tenants_resp.push(Tenant {
            id: tenant_id,
            name: tenant_name,
            slug,
            role: "admin".to_string(),
        });
    }

    let user = User {
        id: user_id,
        email: req.email,
        display_name: req.display_name,
        password_hash: pw_hash,
        tenants: tenants_claim,
    };
    let access_token = issue_jwt(&state.config.jwt_secret, &user)?;
    Ok(Json(LoginResponse {
        access_token,
        token_type: "Bearer",
        user: UserResponse {
            id: user_id,
            email: user.email,
            display_name: user.display_name,
        },
        tenants: tenants_resp,
    }))
}

pub fn generate_slug(name: &str) -> String {
    let base: String = name
        .chars()
        .filter_map(|c| {
            if c.is_alphanumeric() {
                Some(c.to_ascii_lowercase())
            } else if c == ' ' || c == '-' {
                Some('-')
            } else {
                None
            }
        })
        .collect();
    let base = base.trim_matches('-').to_string();
    let suffix: String = (0..3)
        .map(|_| {
            let idx = rand::random::<u8>() % 36;
            if idx < 10 {
                (b'0' + idx) as char
            } else {
                (b'a' + idx - 10) as char
            }
        })
        .collect();
    if base.is_empty() {
        format!("workspace-{suffix}")
    } else {
        format!("{base}-{suffix}")
    }
}

pub async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    let claims = authorize(&state, &headers, None).await?;
    Ok(Json(json!({
        "user": {
            "id": claims.sub,
            "email": claims.email,
            "tenants": claims.tenants
        }
    })))
}

#[derive(Deserialize)]
pub struct ChangePasswordRequest {
    current_password: String,
    new_password: String,
}

pub async fn change_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<ChangePasswordRequest>,
) -> Result<Json<Value>, AppError> {
    let claims = authorize(&state, &headers, None).await?;
    if req.new_password.len() < 8 {
        return Err(AppError::bad_request(
            "password must be at least 8 characters",
        ));
    }
    let user_id = Uuid::parse_str(&claims.sub).unwrap();
    let user = repo::get_user_by_id(&state.pool, user_id)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?
        .ok_or(AppError::not_found("user"))?;
    verify_password(&req.current_password, &user.password_hash)?;
    let new_hash =
        hash_password(&req.new_password).map_err(|e| AppError::internal(e.to_string()))?;
    repo::update_user_password(&state.pool, user_id, &new_hash)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    Ok(Json(json!({"message": "password changed"})))
}
