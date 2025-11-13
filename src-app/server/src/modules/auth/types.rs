// Auth request/response type definitions

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::jwt::TokenPair;
use crate::modules::user::User;

// =====================================================
// Request Types
// =====================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    /// Optional provider name for LDAP/OAuth authentication
    /// If not specified, defaults to local password authentication
    pub provider: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct OAuthAuthorizeQuery {
    /// Redirect URI after successful authentication
    pub redirect_uri: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct OAuthCallbackQuery {
    /// Authorization code from provider
    pub code: String,
    /// State parameter for CSRF protection
    pub state: String,
}

// =====================================================
// Response Types
// =====================================================

#[derive(Debug, Serialize, JsonSchema)]
pub struct AuthResponse {
    pub user: User,
    #[serde(flatten)]
    pub tokens: TokenPair,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct MeResponse {
    pub user: User,
    /// Effective permissions (union of user's direct permissions + all active group permissions)
    pub permissions: Vec<String>,
}
