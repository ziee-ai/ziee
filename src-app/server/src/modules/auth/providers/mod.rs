// Auth providers - infrastructure for future authentication system
#[allow(dead_code)]
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub mod local;
pub mod ldap;
pub mod models;
pub mod oauth2;
pub mod repository;

pub use models::{AuthProvider, OAuthSession};

/// Authentication result containing user info and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResult {
    /// External user ID from the provider
    pub external_id: String,
    /// External username (may differ from internal username)
    pub external_username: Option<String>,
    /// External email address
    pub external_email: Option<String>,
    /// Additional metadata from the provider
    pub metadata: serde_json::Value,
    /// User attributes for provisioning
    pub attributes: UserAttributes,
}

/// User attributes extracted from the provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserAttributes {
    pub username: String,
    pub email: String,
    pub display_name: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub groups: Vec<String>,
}

/// OAuth/OIDC specific result with redirect URL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthResult {
    pub redirect_url: String,
    pub session_key: String,
}

/// Authentication provider trait
#[async_trait]
#[allow(dead_code)]
pub trait AuthProviderTrait: Send + Sync {
    /// Provider name for logging and identification
    fn name(&self) -> &str;

    /// Provider type (local, ldap, oauth2, oidc, saml)
    fn provider_type(&self) -> &str;

    /// Authenticate user with username and password (for password-based providers)
    async fn authenticate(
        &self,
        username: &str,
        password: &str,
    ) -> Result<AuthResult, AuthError>;

    /// Initialize OAuth/OIDC authentication flow (returns redirect URL)
    async fn init_oauth_flow(
        &self,
        _redirect_uri: &str,
    ) -> Result<OAuthResult, AuthError> {
        Err(AuthError::NotSupported("OAuth not supported by this provider".to_string()))
    }

    /// Handle OAuth callback and complete authentication
    async fn handle_oauth_callback(
        &self,
        _code: &str,
        _state: &str,
        _session_key: &str,
    ) -> Result<AuthResult, AuthError> {
        Err(AuthError::NotSupported("OAuth not supported by this provider".to_string()))
    }

    /// Test provider connection (for admin testing)
    async fn test_connection(&self) -> Result<(), AuthError>;

    /// Get provider configuration as JSON
    fn get_config(&self) -> &serde_json::Value;
}

/// Authentication errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthError {
    InvalidCredentials(String),
    ConnectionFailed(String),
    ConfigurationError(String),
    NotSupported(String),
    UserNotFound(String),
    ProviderDisabled(String),
    InternalError(String),
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::InvalidCredentials(msg) => write!(f, "Invalid credentials: {}", msg),
            AuthError::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),
            AuthError::ConfigurationError(msg) => write!(f, "Configuration error: {}", msg),
            AuthError::NotSupported(msg) => write!(f, "Not supported: {}", msg),
            AuthError::UserNotFound(msg) => write!(f, "User not found: {}", msg),
            AuthError::ProviderDisabled(msg) => write!(f, "Provider disabled: {}", msg),
            AuthError::InternalError(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for AuthError {}

/// Provider factory - creates provider instances from database configuration
pub fn create_provider(
    config: &AuthProvider,
    pool: sqlx::PgPool,
) -> Result<Box<dyn AuthProviderTrait>, AuthError> {
    if !config.enabled {
        return Err(AuthError::ProviderDisabled(format!(
            "Provider '{}' is disabled",
            config.name
        )));
    }

    match config.provider_type.as_str() {
        "local" => Ok(Box::new(local::LocalAuthProvider::new(config, pool)?)),
        "ldap" => Ok(Box::new(ldap::LdapAuthProvider::new(config, pool)?)),
        "oauth2" | "oidc" => Ok(Box::new(oauth2::OAuth2Provider::new(config, pool)?)),
        _ => Err(AuthError::ConfigurationError(format!(
            "Unknown provider type: {}",
            config.provider_type
        ))),
    }
}
