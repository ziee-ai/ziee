//! Policy enforcement at MCP user-server write time.
//!
//! Called from BOTH the regular `POST /api/mcp/servers` handler
//! (`mcp/handlers/user.rs::create_user_server`) and the hub
//! user-install handler (`hub/handlers.rs::create_mcp_server_from_hub`).
//! Keeping the logic here means a user can't bypass the gate by going
//! through the hub.
//!
//! Rules:
//!   1. The requested transport MUST be in `policy.allowed_transports`.
//!     422 `MCP_TRANSPORT_NOT_ALLOWED` otherwise.
//!   2. For Stdio:
//!     - `run_in_sandbox` is force-set to `true` (any client value
//!       ignored — security).
//!     - `sandbox_flavor` is force-set to
//!       `policy.user_stdio_sandbox_flavor` (must be Some by the
//!       policy save-time invariant; we 500 if it's None at enforce
//!       time, indicating data corruption).
//!     - If `code_sandbox::config::get_state()` is None at enforce
//!       time, 422 `MCP_SANDBOX_DISABLED` so the user gets a clear
//!       message instead of a confusing connect failure later.

use crate::common::AppError;
use crate::modules::code_sandbox;
use crate::modules::mcp::models::TransportType;
use crate::modules::mcp::types::{CreateMcpServerRequest, UpdateMcpServerRequest};

use super::types::McpUserPolicy;

/// Validate a fresh user-create request against the active policy.
/// Mutates `request` to force the sandbox flag + flavor on stdio.
pub fn enforce_on_user_create(
    request: &mut CreateMcpServerRequest,
    policy: &McpUserPolicy,
) -> Result<(), AppError> {
    let transport_key = transport_key(&request.transport_type);
    if !policy.allowed_transports.iter().any(|t| t == transport_key) {
        return Err(AppError::unprocessable_entity(
            "MCP_TRANSPORT_NOT_ALLOWED",
            format!(
                "Administrator policy does not permit user MCP servers with transport \
                 {transport_key:?}. Allowed: {:?}",
                policy.allowed_transports
            ),
        ));
    }

    if request.transport_type == TransportType::Stdio {
        require_sandbox_state()?;
        let flavor = policy.user_stdio_sandbox_flavor.clone().ok_or_else(|| {
            AppError::internal_error(
                "Policy invariant violated: 'stdio' allowed but no \
                 user_stdio_sandbox_flavor set. Re-save the policy to repair.",
            )
        })?;
        request.run_in_sandbox = Some(true);
        request.sandbox_flavor = Some(flavor);
    }

    Ok(())
}

/// Validate a user-update request that COULD change runtime
/// configuration. Stdio servers continue to inherit the policy's
/// flavor on every update (so a flavor change in the policy
/// propagates the next time the user touches the server). Transport
/// itself is immutable on update (the drawer locks the field), so
/// we don't re-check the allow-list here.
///
/// `current_transport` is the persisted server's transport_type
/// (we don't trust an UpdateMcpServerRequest for that).
pub fn enforce_on_user_transport_change(
    request: &mut UpdateMcpServerRequest,
    current_transport: &TransportType,
    policy: &McpUserPolicy,
) -> Result<(), AppError> {
    if *current_transport == TransportType::Stdio {
        require_sandbox_state()?;
        let flavor = policy.user_stdio_sandbox_flavor.clone().ok_or_else(|| {
            AppError::internal_error(
                "Policy invariant violated: existing user stdio server but \
                 no user_stdio_sandbox_flavor set in policy.",
            )
        })?;
        request.run_in_sandbox = Some(true);
        request.sandbox_flavor = Some(flavor);
    }
    Ok(())
}

fn require_sandbox_state() -> Result<(), AppError> {
    if code_sandbox::config::get_state().is_none() {
        return Err(AppError::unprocessable_entity(
            "MCP_SANDBOX_DISABLED",
            "Stdio MCP servers require the code_sandbox to be enabled in \
             this deployment (it currently is not). Ask your administrator \
             to enable code_sandbox or to remove 'stdio' from the \
             MCP user policy.",
        ));
    }
    Ok(())
}

fn transport_key(transport: &TransportType) -> &'static str {
    match transport {
        TransportType::Stdio => "stdio",
        TransportType::Http => "http",
        TransportType::Sse => "sse",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn policy(allowed: &[&str], flavor: Option<&str>) -> McpUserPolicy {
        McpUserPolicy {
            allowed_transports: allowed.iter().map(|s| (*s).to_string()).collect(),
            user_stdio_sandbox_flavor: flavor.map(str::to_string),
            updated_at: Utc::now(),
            updated_by: None,
        }
    }

    fn req(transport: TransportType) -> CreateMcpServerRequest {
        CreateMcpServerRequest {
            name: "n".into(),
            display_name: "N".into(),
            description: None,
            enabled: None,
            transport_type: transport,
            command: None,
            args: None,
            environment_variables_entries: None,
            url: None,
            headers_entries: None,
            timeout_seconds: None,
            supports_sampling: None,
            usage_mode: None,
            max_concurrent_sessions: None,
            run_in_sandbox: Some(false), // client tries to skip sandbox; we override
            sandbox_flavor: Some("minimal".into()),
            hub_id: None,
        }
    }

    #[test]
    fn rejects_http_when_only_http_disallowed() {
        let mut r = req(TransportType::Http);
        let p = policy(&[], None);
        let err = enforce_on_user_create(&mut r, &p).unwrap_err();
        assert_eq!(err.error_code(), "MCP_TRANSPORT_NOT_ALLOWED");
    }

    #[test]
    fn allows_http_when_in_policy() {
        let mut r = req(TransportType::Http);
        let p = policy(&["http"], None);
        enforce_on_user_create(&mut r, &p).unwrap();
        // Http branch leaves sandbox fields untouched.
        assert_eq!(r.run_in_sandbox, Some(false));
        assert_eq!(r.sandbox_flavor, Some("minimal".into()));
    }

    #[test]
    fn rejects_stdio_when_not_in_policy() {
        let mut r = req(TransportType::Stdio);
        let p = policy(&["http"], None);
        let err = enforce_on_user_create(&mut r, &p).unwrap_err();
        assert_eq!(err.error_code(), "MCP_TRANSPORT_NOT_ALLOWED");
    }

    // The stdio-success path can't run without a live sandbox state;
    // covered by Tier-2 integration. The internal_error invariant
    // branch can't be triggered without poking the policy directly.

    fn upd() -> UpdateMcpServerRequest {
        UpdateMcpServerRequest {
            name: None,
            display_name: None,
            description: None,
            enabled: None,
            command: None,
            args: None,
            environment_variables_entries: None,
            url: None,
            headers_entries: None,
            timeout_seconds: None,
            supports_sampling: None,
            usage_mode: None,
            max_concurrent_sessions: None,
            run_in_sandbox: Some(false), // user tries to opt out
            sandbox_flavor: None,
        }
    }

    #[test]
    fn update_leaves_http_untouched() {
        let mut r = upd();
        let p = policy(&["http", "stdio"], Some("full"));
        enforce_on_user_transport_change(&mut r, &TransportType::Http, &p).unwrap();
        assert_eq!(r.run_in_sandbox, Some(false));
        assert_eq!(r.sandbox_flavor, None);
    }

    // Silences dead_code warnings if used only via integration tests.
    fn _unused(_: Uuid) {}
}
