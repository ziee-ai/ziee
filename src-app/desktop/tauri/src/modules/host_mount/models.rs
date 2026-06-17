//! Request/response DTOs + DB row shapes for the host-mount feature.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}

/// One mounted host folder (stored as a JSONB array element on `host_mounts`).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MountEntry {
    /// Absolute path on the user's machine (the desktop host).
    pub host_path: String,
    /// Read-only by default; read-write additionally requires the policy's
    /// `allow_readwrite`.
    #[serde(default = "default_true")]
    pub read_only: bool,
}

/// GET response / PUT body for a scope's (conversation or project) mount list.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HostMountsBody {
    pub mounts: Vec<MountEntry>,
}

/// Deployment policy (singleton) — GET response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HostMountPolicyResponse {
    pub enabled: bool,
    pub allowed_prefixes: Vec<String>,
    pub allow_readwrite: bool,
}

/// Deployment policy — PUT body (tri-state-free: omitted field = unchanged).
#[derive(Debug, Clone, Deserialize, JsonSchema, Default)]
pub struct UpdateHostMountPolicyRequest {
    pub enabled: Option<bool>,
    pub allowed_prefixes: Option<Vec<String>>,
    pub allow_readwrite: Option<bool>,
}

/// Internal policy row (sqlx).
#[derive(Debug, Clone)]
pub struct HostMountPolicyRow {
    pub enabled: bool,
    pub allowed_prefixes: Vec<String>,
    pub allow_readwrite: bool,
}

impl From<HostMountPolicyRow> for HostMountPolicyResponse {
    fn from(r: HostMountPolicyRow) -> Self {
        Self {
            enabled: r.enabled,
            allowed_prefixes: r.allowed_prefixes,
            allow_readwrite: r.allow_readwrite,
        }
    }
}
