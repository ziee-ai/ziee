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

/// Readiness report returned by the admin `[Connect]` installer flow (ITEM-13).
///
/// The `[Connect]` action runs the one-shot install steps (trust the bridge CA,
/// register the add-in manifest for sideloading) and reports where the host
/// ended up. Every step is best-effort: a failed step sets its boolean `false`
/// and appends a human-readable note to `message` rather than failing the whole
/// request, so the admin sees a partial-success report instead of a 500.
///
/// Like [`OfficeBridgeSettings`], this DTO must NEVER carry the bridge's
/// per-session token or any secret — it is display/diagnostic state only.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConnectReadiness {
    /// Whether a Microsoft Office installation was detected on the host.
    pub office_present: bool,
    /// True ⇒ warn the user: Office is running elevated (as administrator), so
    /// the add-in platform is disabled and the bridge cannot attach. Office must
    /// be restarted without administrator rights.
    pub office_elevated_warning: bool,
    /// Whether the bridge CA was successfully installed into the OS trust store
    /// (one UAC prompt on Windows). False ⇒ see `message`.
    pub cert_trusted: bool,
    /// Whether the add-in manifest was registered for sideloading. False ⇒ see
    /// `message`.
    pub sideloaded: bool,
    /// The TCP port the bridge HTTPS+WSS listener uses (echoed for the UI).
    pub bridge_port: i32,
    /// Human-readable summary of the outcome — a success line when every step
    /// landed, else the concatenated per-step failure/warning notes.
    pub message: String,
}
