use crate::common::AppError;
use axum::http::StatusCode;

// User group module error codes
#[allow(dead_code)]
pub const GROUP_NOT_FOUND: &str = "GROUP_NOT_FOUND";
#[allow(dead_code)]
pub const GROUP_ALREADY_EXISTS: &str = "GROUP_ALREADY_EXISTS";
#[allow(dead_code)]
pub const GROUP_CREATION_FAILED: &str = "GROUP_CREATION_FAILED";
#[allow(dead_code)]
pub const GROUP_UPDATE_FAILED: &str = "GROUP_UPDATE_FAILED";
#[allow(dead_code)]
pub const GROUP_DELETION_FAILED: &str = "GROUP_DELETION_FAILED";
#[allow(dead_code)]
pub const GROUP_PROTECTED: &str = "GROUP_PROTECTED";

// Convenience error constructors for user_group module
#[allow(dead_code)]
pub fn group_not_found() -> AppError {
    AppError::new(StatusCode::NOT_FOUND, GROUP_NOT_FOUND, "Group not found")
}

#[allow(dead_code)]
pub fn group_already_exists() -> AppError {
    AppError::new(
        StatusCode::CONFLICT,
        GROUP_ALREADY_EXISTS,
        "Group name already exists",
    )
}

#[allow(dead_code)]
pub fn group_protected() -> AppError {
    AppError::new(
        StatusCode::FORBIDDEN,
        GROUP_PROTECTED,
        "Cannot modify protected group",
    )
}
