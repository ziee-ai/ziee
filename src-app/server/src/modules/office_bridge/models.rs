//! Request / response DTOs for the office_bridge admin REST surface.
//!
//! NOTE: this struct is serialized to the admin API — it must NEVER carry the
//! bridge's per-session token or any secret. `cert_fingerprint` is a public
//! (non-secret) certificate fingerprint used only for display/diagnostics.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Deployment-wide office-bridge settings (singleton row). Returned by GET.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OfficeBridgeSettings {
    /// Runtime admin toggle (distinct from the deploy-level config kill switch).
    pub enabled: bool,
    /// Fixed TCP port the bridge HTTPS+WSS listener binds (default 44300).
    pub port: i32,
    /// Last time a task pane successfully connected, or null if never.
    pub last_connected_at: Option<DateTime<Utc>>,
    /// Public fingerprint of the locally-trusted bridge cert (not a secret).
    pub cert_fingerprint: Option<String>,
}

/// PUT body for the global settings. Every field optional → absent = leave.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct UpdateOfficeBridgeSettingsRequest {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub port: Option<i32>,
}
