use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Auth provider configuration - matches new schema
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AuthProvider {
    pub id: Uuid,
    pub name: String,
    pub provider_type: String,
    pub enabled: bool,
    pub config: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// OAuth session for OAuth/OIDC flows - matches new schema
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct OAuthSession {
    pub id: Uuid,
    pub state: String,
    pub provider_id: Uuid,
    pub pkce_verifier: Option<String>,
    pub nonce: Option<String>,
    pub redirect_uri: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

/// User auth link (external auth provider linkage) - matches new schema
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserAuthLink {
    pub id: Uuid,
    pub user_id: Uuid,
    pub provider_id: Uuid,
    pub external_id: String,
    pub external_email: Option<String>,
    pub external_data: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_login_at: Option<DateTime<Utc>>,
}
