use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// =====================================================
// Request/Response Types
// =====================================================

/// Setup-status response shown to unauthenticated callers.
///
/// Closes 13-misc F-02 (Medium): the original response leaked the
/// app name + version to every anonymous fetch, which is fingerprint
/// material attackers use to pick a known-CVE matrix. We now expose
/// only the single bit the client actually needs (`needs_setup`).
/// app_name / version remain available to authenticated callers via
/// a separate endpoint if needed.
#[derive(Debug, Serialize, JsonSchema)]
pub struct SetupStatusResponse {
    pub needs_setup: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetupAdminRequest {
    pub username: String,
    pub email: String,
    pub password: String,
    pub display_name: Option<String>,
}
