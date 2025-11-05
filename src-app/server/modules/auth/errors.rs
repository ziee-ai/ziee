use crate::common::AppError;
use axum::http::StatusCode;

// Auth module error codes
#[allow(dead_code)]
pub const AUTH_INVALID_CREDENTIALS: &str = "AUTH_INVALID_CREDENTIALS";
pub const AUTH_MISSING_TOKEN: &str = "AUTH_MISSING_TOKEN";
pub const AUTH_INVALID_TOKEN: &str = "AUTH_INVALID_TOKEN";
#[allow(dead_code)]
pub const AUTH_TOKEN_EXPIRED: &str = "AUTH_TOKEN_EXPIRED";
#[allow(dead_code)]
pub const AUTH_TOKEN_GENERATION_FAILED: &str = "AUTH_TOKEN_GENERATION_FAILED";

// Convenience error constructors for auth module
#[allow(dead_code)]
pub fn invalid_credentials() -> AppError {
    AppError::new(
        StatusCode::UNAUTHORIZED,
        AUTH_INVALID_CREDENTIALS,
        "Invalid credentials",
    )
}

pub fn missing_token() -> AppError {
    AppError::new(
        StatusCode::UNAUTHORIZED,
        AUTH_MISSING_TOKEN,
        "Missing or invalid authorization header",
    )
}

pub fn invalid_token() -> AppError {
    AppError::new(
        StatusCode::UNAUTHORIZED,
        AUTH_INVALID_TOKEN,
        "Invalid or expired token",
    )
}

#[allow(dead_code)]
pub fn token_generation_failed() -> AppError {
    AppError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        AUTH_TOKEN_GENERATION_FAILED,
        "Failed to generate authentication token",
    )
}
