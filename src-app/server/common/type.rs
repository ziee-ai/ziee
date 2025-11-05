use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

// =====================================================
// API Result Type
// =====================================================

/// API result type that includes HTTP status code
/// This is the standard return type for all API handlers
///
/// # Examples
///
/// ```rust
/// use axum::{http::StatusCode, Json};
/// use crate::common::type::ApiResult;
///
/// async fn my_handler() -> ApiResult<Json<MyResponse>> {
///     Ok((StatusCode::OK, Json(MyResponse { /* ... */ })))
/// }
/// ```
pub type ApiResult<T> = Result<(StatusCode, T), (StatusCode, AppError)>;

// =====================================================
// Error Types
// =====================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiError {
    pub error: String,
    pub error_code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppError {
    status_code: u16,
    error_code: String,
    message: String,
    details: Option<serde_json::Value>,
}

impl AppError {
    pub fn new(status_code: StatusCode, error_code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            status_code: status_code.as_u16(),
            error_code: error_code.into(),
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    // Common convenience constructors
    pub fn not_found(resource: &str) -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            "RESOURCE_NOT_FOUND",
            format!("{} not found", resource),
        )
    }

    pub fn conflict(resource: &str) -> Self {
        Self::new(
            StatusCode::CONFLICT,
            "RESOURCE_CONFLICT",
            format!("{} already exists", resource),
        )
    }

    pub fn bad_request(error_code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, error_code, message)
    }

    pub fn unprocessable_entity(error_code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNPROCESSABLE_ENTITY, error_code, message)
    }

    pub fn unauthorized(error_code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, error_code, message)
    }

    pub fn forbidden(error_code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, error_code, message)
    }

    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "SYSTEM_INTERNAL_ERROR", message)
    }

    pub fn database_error(err: impl std::error::Error) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "SYSTEM_DATABASE_ERROR",
            format!("Database error: {}", err),
        )
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for AppError {}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let body = Json(ApiError {
            error: self.message,
            error_code: self.error_code,
            details: self.details,
        });

        let status = StatusCode::from_u16(self.status_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (status, body).into_response()
    }
}

// Conversion from common error types
impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => AppError::not_found("Resource"),
            _ => AppError::database_error(err),
        }
    }
}

impl From<Box<dyn std::error::Error + Send + Sync>> for AppError {
    fn from(err: Box<dyn std::error::Error + Send + Sync>) -> Self {
        AppError::internal_error(err.to_string())
    }
}

// Helper conversions for ApiResult
impl From<AppError> for (StatusCode, AppError) {
    fn from(err: AppError) -> Self {
        let status = StatusCode::from_u16(err.status_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (status, err)
    }
}

// Helper to convert sqlx errors to ApiResult error type
impl AppError {
    pub fn to_api_error(self) -> (StatusCode, Self) {
        let status = StatusCode::from_u16(self.status_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (status, self)
    }
}

// =====================================================
// Common Types
// =====================================================

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct PaginationQuery {
    #[serde(default = "default_page")]
    pub page: i32,
    #[serde(default = "default_per_page")]
    pub per_page: i32,
}

fn default_page() -> i32 {
    1
}

fn default_per_page() -> i32 {
    20
}

impl Default for PaginationQuery {
    fn default() -> Self {
        Self {
            page: 1,
            per_page: 20,
        }
    }
}
