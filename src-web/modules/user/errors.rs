use crate::common::AppError;
use axum::http::StatusCode;

// User module error codes
#[allow(dead_code)]
pub const USER_NOT_FOUND: &str = "USER_NOT_FOUND";
#[allow(dead_code)]
pub const USER_ALREADY_EXISTS: &str = "USER_ALREADY_EXISTS";
#[allow(dead_code)]
pub const USER_CREATION_FAILED: &str = "USER_CREATION_FAILED";
#[allow(dead_code)]
pub const USER_UPDATE_FAILED: &str = "USER_UPDATE_FAILED";
#[allow(dead_code)]
pub const USER_DELETION_FAILED: &str = "USER_DELETION_FAILED";
#[allow(dead_code)]
pub const USER_PROTECTED: &str = "USER_PROTECTED";
#[allow(dead_code)]
pub const USER_INACTIVE: &str = "USER_INACTIVE";
#[allow(dead_code)]
pub const INVALID_PASSWORD: &str = "INVALID_PASSWORD";
#[allow(dead_code)]
pub const NO_PASSWORD_SERVICE: &str = "NO_PASSWORD_SERVICE";

// Convenience error constructors for user module
#[allow(dead_code)]
pub fn user_not_found() -> AppError {
    AppError::new(StatusCode::NOT_FOUND, USER_NOT_FOUND, "User not found")
}

#[allow(dead_code)]
pub fn user_already_exists(field: &str) -> AppError {
    AppError::new(
        StatusCode::CONFLICT,
        USER_ALREADY_EXISTS,
        format!("{} already exists", field),
    )
}

#[allow(dead_code)]
pub fn user_protected() -> AppError {
    AppError::new(
        StatusCode::FORBIDDEN,
        USER_PROTECTED,
        "Cannot delete protected user",
    )
}

#[allow(dead_code)]
pub fn user_inactive() -> AppError {
    AppError::new(StatusCode::FORBIDDEN, USER_INACTIVE, "User is not active")
}

#[allow(dead_code)]
pub fn invalid_password() -> AppError {
    AppError::new(
        StatusCode::UNAUTHORIZED,
        INVALID_PASSWORD,
        "Invalid password",
    )
}

#[allow(dead_code)]
pub fn no_password_service() -> AppError {
    AppError::new(
        StatusCode::BAD_REQUEST,
        NO_PASSWORD_SERVICE,
        "User has no password service",
    )
}
