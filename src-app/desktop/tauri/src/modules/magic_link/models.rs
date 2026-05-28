//! Request/response DTOs for the magic-link endpoints.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Body of POST /api/auth/magic-link/exchange.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct MagicLinkExchangeRequest {
    pub token: String,
}

/// Response of POST /api/auth/magic-link/issue.
///
/// `token` is the plaintext — returned ONCE and never persisted.
/// The desktop UI encodes it into the QR URL
/// `https://<tunnel>/auth/magic/<token>`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MagicLinkIssueResponse {
    pub token: String,
    pub expires_at: DateTime<Utc>,
}

/// DB row from magic_link_tokens.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MagicLinkRow {
    pub token_hash: String,
    pub user_id: Uuid,
    pub expires_at: DateTime<Utc>,
    pub used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}
