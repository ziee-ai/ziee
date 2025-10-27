use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::modules::user::User;

// Login request
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LoginRequest {
    pub username_or_email: String,
    pub password: String,
}

// Login response
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LoginResponse {
    pub token: String,
    pub user: User,
    pub expires_at: DateTime<Utc>,
}

// Logout request
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LogoutRequest {
    pub token: String,
}

// Token validation request
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ValidateTokenRequest {
    pub token: String,
}

// Current user response
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CurrentUserResponse {
    pub user: User,
}

// Register request (alias for CreateUserRequest)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
    pub profile: Option<serde_json::Value>,
}
