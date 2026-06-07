// MCP handlers module
// Organizes all handler functions for MCP server operations

pub mod groups;
pub mod runtime;
pub mod system;
pub mod test_connection;
pub mod user;

// Re-export user handlers
pub use user::*;

// Re-export system handlers
pub use system::*;

// Re-export group assignment handlers
pub use groups::*;

// Runtime handlers are accessed via runtime:: prefix in routes

use crate::common::AppError;
use crate::modules::mcp::client::stdio::HOST_ALLOWED_COMMANDS;
use crate::modules::mcp::models::{McpServer, TransportType};
use crate::modules::mcp::types::{CreateMcpServerRequest, UpdateMcpServerRequest};

/// A stdio command not run in the sandbox must be one of the launchers
/// the host path can resolve to the bundled bun/uv runtimes.
fn require_host_command(cmd: &str) -> Result<(), AppError> {
    if !HOST_ALLOWED_COMMANDS.contains(&cmd) {
        return Err(AppError::bad_request(
            "INVALID_COMMAND",
            format!(
                "Command '{}' is not allowed on the host. Allowed commands: {:?}. \
                 Enable run-in-sandbox to use any command.",
                cmd, HOST_ALLOWED_COMMANDS
            ),
        ));
    }
    Ok(())
}

/// Reject an unknown `sandbox_flavor` (must be one of KNOWN_FLAVORS).
fn validate_sandbox_flavor(flavor: Option<&str>) -> Result<(), AppError> {
    if let Some(f) = flavor {
        let known = crate::modules::code_sandbox::types::KNOWN_FLAVORS
            .iter()
            .any(|m| m.flavor == f);
        if !known {
            return Err(AppError::bad_request(
                "INVALID_FLAVOR",
                format!("Unknown sandbox flavor '{}'", f),
            ));
        }
    }
    Ok(())
}

/// Tiered command + flavor validation for **create**. A stdio server
/// that won't run in the sandbox (user-owned, or a system server with
/// run_in_sandbox off) must use a host-allowed command; a sandboxed
/// server may use any command (bwrap isolation is the guard).
pub(crate) fn validate_sandbox_fields_create(
    is_system: bool,
    req: &CreateMcpServerRequest,
) -> Result<(), AppError> {
    validate_sandbox_flavor(req.sandbox_flavor.as_deref())?;
    let sandboxed = is_system
        && req.transport_type == TransportType::Stdio
        && req.run_in_sandbox.unwrap_or(false);
    if req.transport_type == TransportType::Stdio && !sandboxed {
        if let Some(cmd) = req.command.as_deref() {
            require_host_command(cmd)?;
        }
    }
    Ok(())
}

/// Tiered command + flavor validation for **update**, using the existing
/// row to fill in fields the request omits (transport is immutable;
/// command / run_in_sandbox fall back to the persisted values).
pub(crate) fn validate_sandbox_fields_update(
    existing: &McpServer,
    req: &UpdateMcpServerRequest,
) -> Result<(), AppError> {
    validate_sandbox_flavor(req.sandbox_flavor.as_deref())?;
    let command = req.command.as_deref().or(existing.command.as_deref());
    let run_in_sandbox = req.run_in_sandbox.unwrap_or(existing.run_in_sandbox);
    let sandboxed =
        existing.is_system && existing.transport_type == TransportType::Stdio && run_in_sandbox;
    if existing.transport_type == TransportType::Stdio && !sandboxed {
        if let Some(cmd) = command {
            require_host_command(cmd)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_allowlist_accepts_the_five_launchers_rejects_others() {
        for c in ["npx", "uvx", "python", "python3", "node"] {
            assert!(require_host_command(c).is_ok(), "{c} should be allowed");
        }
        // deno dropped; arbitrary binaries rejected on the host path.
        for c in ["deno", "bash", "sh", "rm", "Rscript", "custom-bin"] {
            assert!(require_host_command(c).is_err(), "{c} should be rejected");
        }
    }

    #[test]
    fn sandbox_flavor_must_be_known_or_absent() {
        assert!(validate_sandbox_flavor(None).is_ok());
        assert!(validate_sandbox_flavor(Some("full")).is_ok());
        assert!(validate_sandbox_flavor(Some("minimal")).is_ok());
        assert!(validate_sandbox_flavor(Some("bogus")).is_err());
        assert!(validate_sandbox_flavor(Some("")).is_err());
    }
}
