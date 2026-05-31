mod auth;
mod cdp;
mod config;
mod error;
mod handlers;
mod state;
mod types;
mod ws;

pub use config::AppConfig;
pub use state::AppState;

use axum::{
    routing::{get, post},
    Router,
};
use tower_http::{
    cors::CorsLayer,
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(handlers::health::health))
        .route("/ready", get(handlers::health::ready))
        .route("/metrics", get(handlers::health::metrics))
        .route("/api/v1/auth/login", post(auth::handlers::login))
        .route("/api/v1/auth/signup", post(auth::handlers::signup))
        .route("/api/v1/auth/me", get(auth::handlers::me))
        .route(
            "/api/v1/auth/change-password",
            post(auth::handlers::change_password),
        )
        .route(
            "/api/v1/agents/register",
            post(handlers::agents::agent_register),
        )
        .route("/api/v1/agents/connect", get(ws::agent::ws_agent))
        .route(
            "/api/v1/tenants/:tenant_id/browser-instances",
            get(handlers::browsers::list_browsers),
        )
        .route(
            "/api/v1/tenants/:tenant_id/browser-instances/:browser_id",
            get(handlers::browsers::get_browser).delete(handlers::browsers::delete_browser),
        )
        .route(
            "/api/v1/tenants/:tenant_id/browser-instances/:browser_id/reset",
            post(handlers::browsers::reset_browser),
        )
        .route(
            "/api/v1/tenants/:tenant_id/agents",
            get(handlers::agents::list_agents),
        )
        .route(
            "/api/v1/tenants/:tenant_id/agents/:agent_id",
            get(handlers::agents::get_agent).delete(handlers::agents::delete_agent),
        )
        .route(
            "/api/v1/tenants/:tenant_id/tokens/cdp",
            get(handlers::tokens::list_cdp_tokens).post(handlers::tokens::create_cdp_token),
        )
        .route(
            "/api/v1/tenants/:tenant_id/tokens/cdp/:token_id/revoke",
            post(handlers::tokens::revoke_token_handler),
        )
        .route(
            "/api/v1/tenants/:tenant_id/tokens/cdp/:token_id/rotate",
            post(handlers::tokens::rotate_token),
        )
        .route(
            "/api/v1/tenants/:tenant_id/agent-registration-tokens",
            get(handlers::tokens::list_agent_registration_tokens)
                .post(handlers::tokens::create_agent_registration_token),
        )
        .route(
            "/api/v1/tenants/:tenant_id/agent-registration-tokens/:token_id/revoke",
            post(handlers::tokens::revoke_token_handler),
        )
        .route(
            "/api/v1/tenants/:tenant_id/audit-logs",
            get(handlers::audit::list_audit_logs),
        )
        .route(
            "/api/v1/tenants/:tenant_id/invitations",
            get(handlers::invitations::list_invitations)
                .post(handlers::invitations::create_invitation),
        )
        .route(
            "/api/v1/tenants/:tenant_id/invitations/:invitation_id",
            axum::routing::delete(handlers::invitations::delete_invitation_handler),
        )
        .route(
            "/api/v1/invitations/:token/validate",
            get(handlers::invitations::validate_invitation),
        )
        .route(
            "/api/v1/invitations/:token/accept",
            post(handlers::invitations::accept_invitation),
        )
        .route(
            "/api/v1/tenants/:tenant_id/members",
            get(handlers::members::list_members_handler),
        )
        .route(
            "/api/v1/tenants/:tenant_id/members/leave",
            post(handlers::members::leave_tenant),
        )
        .route(
            "/api/v1/tenants/:tenant_id/members/:user_id",
            axum::routing::patch(handlers::members::update_member_role_handler)
                .delete(handlers::members::remove_member),
        )
        .route(
            "/api/v1/tenants/:tenant_id",
            axum::routing::patch(handlers::tenants::update_tenant_handler)
                .delete(handlers::tenants::delete_tenant_handler),
        )
        .route(
            "/api/v1/tenants",
            post(handlers::tenants::create_tenant_handler),
        )
        .route("/ws/control", get(ws::control::ws_control))
        .route(
            "/cdp/tenants/:tenant_id/browser-instances/:browser_id/json/version",
            get(cdp::cdp_json_version),
        )
        .route(
            "/cdp/tenants/:tenant_id/browser-instances/:browser_id/json/list",
            get(cdp::cdp_json_list),
        )
        .route(
            "/cdp/tenants/:tenant_id/browser-instances/:browser_id/devtools/browser/:target_id",
            get(cdp::cdp_ws_tunnel),
        )
        .route(
            "/cdp/tenants/:tenant_id/browser-instances/:browser_id/devtools/page/:target_id",
            get(cdp::cdp_ws_tunnel),
        )
        .fallback_service(
            ServeDir::new("frontend/dist").fallback(ServeFile::new("frontend/dist/index.html")),
        )
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::auth::crypto::*;
    use super::types::*;
    use uuid::Uuid;

    #[test]
    fn jwt_round_trips_tenant_claims() {
        let user = User {
            id: Uuid::now_v7(),
            email: "admin@example.com".to_string(),
            display_name: "Admin".to_string(),
            password_hash: "unused".to_string(),
            tenants: vec![TenantClaim {
                id: Uuid::now_v7(),
                role: "admin".to_string(),
            }],
        };

        let token = issue_jwt("test-secret", &user).unwrap();
        let claims = decode_jwt("test-secret", &token).unwrap();

        assert_eq!(claims.email, user.email);
        assert_eq!(claims.tenants[0].id, user.tenants[0].id);
    }

    #[test]
    fn token_hash_is_stable_and_not_plaintext() {
        let token = "jbr_cdp_secret";
        let hash = hash_token(token);

        assert_eq!(hash, hash_token(token));
        assert_ne!(hash, token);
    }

    #[test]
    fn generate_slug_basic() {
        let slug = super::auth::handlers::generate_slug("Alice Smith");
        assert!(slug.starts_with("alice-smith-"));
        assert_eq!(slug.len(), "alice-smith-".len() + 3);
    }

    #[test]
    fn generate_slug_empty() {
        let slug = super::auth::handlers::generate_slug("");
        assert!(slug.starts_with("workspace-"));
    }

    #[test]
    fn generate_slug_special_chars() {
        let slug = super::auth::handlers::generate_slug("Hello! @World#");
        assert!(slug.starts_with("hello-world-"));
    }
}
