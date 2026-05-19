use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

use persistence::DbError;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("unauthorized")]
    Unauthorized,
    #[error("not found")]
    NotFound,
    #[error("conflict")]
    Conflict,
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<DbError> for ApiError {
    fn from(e: DbError) -> Self {
        match e {
            DbError::NotFound => ApiError::NotFound,
            DbError::Conflict => ApiError::Conflict,
            DbError::Sqlx(err) => ApiError::Internal(err.to_string()),
        }
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        ApiError::Internal(e.to_string())
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        ApiError::Internal(e.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, msg) = match &self {
            ApiError::BadRequest(m) => (StatusCode::BAD_REQUEST, "BAD_REQUEST", m.clone()),
            ApiError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "UNAUTHORIZED",
                "missing or invalid credentials".to_string(),
            ),
            ApiError::NotFound => (StatusCode::NOT_FOUND, "NOT_FOUND", "not found".to_string()),
            ApiError::Conflict => (StatusCode::CONFLICT, "CONFLICT", "conflict".to_string()),
            ApiError::Internal(m) => {
                tracing::error!(error = %m, "internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL",
                    "internal error".to_string(),
                )
            }
        };

        let body = Json(json!({
            "error": { "code": code, "message": msg }
        }));
        (status, body).into_response()
    }
}

pub type ApiResult<T> = std::result::Result<T, ApiError>;
