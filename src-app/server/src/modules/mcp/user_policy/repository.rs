//! Singleton persistence for `mcp_user_policy`.

use chrono::DateTime;
use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::code_sandbox;

use super::types::{McpUserPolicy, UpdateMcpUserPolicyRequest};

const VALID_USER_TRANSPORTS: &[&str] = &["http", "stdio"];

/// Load the singleton policy row. Returns the row inserted by
/// migration 84 (always present after migrate).
///
/// Projects the live `code_sandbox.enabled` state into the response:
/// if the sandbox is currently disabled, `'stdio'` is filtered out of
/// `allowed_transports` (and `user_stdio_sandbox_flavor` cleared) so
/// the UI's transport dropdown stays in sync with what the create
/// handler will actually accept. Without this projection, users see
/// stdio in the dropdown, pick it, and hit a 422 `MCP_SANDBOX_DISABLED`
/// at submit time — confusing and avoidable. The persisted row is
/// unchanged so the admin's stdio preference is restored automatically
/// when sandbox is re-enabled.
pub async fn load(pool: &PgPool) -> Result<McpUserPolicy, AppError> {
    let row = sqlx::query!(
        r#"SELECT
            allowed_transports        AS "allowed_transports!: Vec<String>",
            user_stdio_sandbox_flavor,
            updated_at,
            updated_by
        FROM mcp_user_policy
        WHERE id = 1"#,
    )
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::internal_error(format!("Failed to load mcp_user_policy: {e}")))?;

    let sandbox_enabled = code_sandbox::config::get_state().is_some();
    let (allowed_transports, flavor) = if sandbox_enabled {
        (row.allowed_transports, row.user_stdio_sandbox_flavor)
    } else {
        let filtered: Vec<String> = row
            .allowed_transports
            .into_iter()
            .filter(|t| t != "stdio")
            .collect();
        (filtered, None)
    };

    Ok(McpUserPolicy {
        allowed_transports,
        user_stdio_sandbox_flavor: flavor,
        updated_at: DateTime::from_timestamp(row.updated_at.unix_timestamp(), 0)
            .ok_or_else(|| AppError::internal_error("invalid updated_at"))?,
        updated_by: row.updated_by,
    })
}

/// Persist the policy after validating it against the live sandbox
/// state. The `updated_by` user id is stamped as a side-effect.
pub async fn save(
    pool: &PgPool,
    updated_by: Uuid,
    req: UpdateMcpUserPolicyRequest,
) -> Result<McpUserPolicy, AppError> {
    let (allowed, flavor) = validate(req)?;

    sqlx::query!(
        r#"UPDATE mcp_user_policy
           SET allowed_transports = $1,
               user_stdio_sandbox_flavor = $2,
               updated_at = now(),
               updated_by = $3
           WHERE id = 1"#,
        &allowed,
        flavor.as_deref(),
        updated_by,
    )
    .execute(pool)
    .await
    .map_err(|e| AppError::internal_error(format!("Failed to update mcp_user_policy: {e}")))?;

    load(pool).await
}

/// Validate a candidate policy. Pure (no DB) except for the live
/// sandbox-state lookup, which is a process-global Lazy and effectively
/// free. Returns `(normalized_allowed_transports, normalized_flavor)`.
fn validate(
    req: UpdateMcpUserPolicyRequest,
) -> Result<(Vec<String>, Option<String>), AppError> {
    // Normalize: dedupe + lowercase, drop empties.
    let mut allowed: Vec<String> = req
        .allowed_transports
        .into_iter()
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    allowed.sort();
    allowed.dedup();

    for transport in &allowed {
        if !VALID_USER_TRANSPORTS.contains(&transport.as_str()) {
            return Err(AppError::unprocessable_entity(
                "MCP_INVALID_TRANSPORT",
                format!(
                    "allowed_transports may only contain {:?}; got {transport:?}",
                    VALID_USER_TRANSPORTS
                ),
            ));
        }
    }

    let flavor = if allowed.iter().any(|t| t == "stdio") {
        // Stdio implies sandbox; flavor required and must be known.
        let f = req
            .user_stdio_sandbox_flavor
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                AppError::unprocessable_entity(
                    "MCP_FLAVOR_REQUIRED",
                    "user_stdio_sandbox_flavor is required when 'stdio' is in allowed_transports",
                )
            })?;

        let known = code_sandbox::types::KNOWN_FLAVORS
            .iter()
            .any(|m| m.flavor == f);
        if !known {
            let names: Vec<&str> = code_sandbox::types::KNOWN_FLAVORS
                .iter()
                .map(|m| m.flavor)
                .collect();
            return Err(AppError::unprocessable_entity(
                "MCP_UNKNOWN_FLAVOR",
                format!("user_stdio_sandbox_flavor must be one of {names:?}; got {f:?}"),
            ));
        }

        if code_sandbox::config::get_state().is_none() {
            return Err(AppError::unprocessable_entity(
                "MCP_SANDBOX_DISABLED",
                "Cannot enable stdio for users: code_sandbox is disabled in this deployment. \
                 Enable code_sandbox in config and restart, or omit 'stdio' from allowed_transports.",
            ));
        }
        Some(f)
    } else {
        // Stdio not allowed → clear the flavor so we don't carry a
        // stale pick when the admin re-enables stdio later.
        None
    };

    Ok((allowed, flavor))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(allowed: &[&str], flavor: Option<&str>) -> UpdateMcpUserPolicyRequest {
        UpdateMcpUserPolicyRequest {
            allowed_transports: allowed.iter().map(|s| (*s).to_string()).collect(),
            user_stdio_sandbox_flavor: flavor.map(str::to_string),
        }
    }

    #[test]
    fn validate_http_only_clears_flavor() {
        let (allowed, flavor) = validate(req(&["http"], Some("full"))).unwrap();
        assert_eq!(allowed, vec!["http"]);
        assert_eq!(flavor, None);
    }

    #[test]
    fn validate_empty_allowed_clears_flavor() {
        let (allowed, flavor) = validate(req(&[], None)).unwrap();
        assert!(allowed.is_empty());
        assert_eq!(flavor, None);
    }

    #[test]
    fn validate_dedupes_and_lowercases() {
        let (allowed, _) = validate(req(&["HTTP", "http", " "], None)).unwrap();
        assert_eq!(allowed, vec!["http"]);
    }

    #[test]
    fn validate_rejects_sse() {
        let err = validate(req(&["sse"], None)).unwrap_err();
        assert_eq!(err.error_code(), "MCP_INVALID_TRANSPORT");
    }

    #[test]
    fn validate_rejects_stdio_without_flavor() {
        let err = validate(req(&["stdio"], None)).unwrap_err();
        assert_eq!(err.error_code(), "MCP_FLAVOR_REQUIRED");
    }

    #[test]
    fn validate_rejects_unknown_flavor() {
        let err = validate(req(&["stdio"], Some("gigantic"))).unwrap_err();
        assert_eq!(err.error_code(), "MCP_UNKNOWN_FLAVOR");
    }

    // The sandbox-disabled branch is exercised in Tier-2 (integration)
    // because it depends on the live process-global sandbox state,
    // which the unit test harness can't toggle cleanly without
    // touching real init paths.
}
