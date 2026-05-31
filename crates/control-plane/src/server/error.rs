use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{detail}")]
    Http {
        status: StatusCode,
        code: &'static str,
        detail: String,
    },
}

impl AppError {
    pub fn unauthorized(detail: impl Into<String>) -> Self {
        Self::Http {
            status: StatusCode::UNAUTHORIZED,
            code: "UNAUTHORIZED",
            detail: detail.into(),
        }
    }

    pub fn forbidden(detail: impl Into<String>) -> Self {
        Self::Http {
            status: StatusCode::FORBIDDEN,
            code: "FORBIDDEN",
            detail: detail.into(),
        }
    }

    pub fn not_found(resource: &'static str) -> Self {
        Self::Http {
            status: StatusCode::NOT_FOUND,
            code: "NOT_FOUND",
            detail: format!("{resource} not found"),
        }
    }

    pub fn bad_request(detail: impl Into<String>) -> Self {
        Self::Http {
            status: StatusCode::BAD_REQUEST,
            code: "BAD_REQUEST",
            detail: detail.into(),
        }
    }

    pub fn internal(detail: impl Into<String>) -> Self {
        Self::Http {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "INTERNAL_ERROR",
            detail: detail.into(),
        }
    }

    pub fn conflict(detail: impl Into<String>) -> Self {
        Self::Http {
            status: StatusCode::CONFLICT,
            code: "CONFLICT",
            detail: detail.into(),
        }
    }

    pub fn service_unavailable(detail: impl Into<String>) -> Self {
        Self::Http {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code: "SERVICE_UNAVAILABLE",
            detail: detail.into(),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::Http {
                status,
                code,
                detail,
            } => (
                status,
                Json(json!({
                    "title": code,
                    "status": status.as_u16(),
                    "detail": detail
                })),
            )
                .into_response(),
        }
    }
}
