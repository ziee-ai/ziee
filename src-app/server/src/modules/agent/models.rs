//! Agent module data types — the deployment-wide policy singleton DTO +
//! its tri-state partial-update request.
//!
//! Mirrors `summarization::models` (nullable tri-state fields) and
//! `js_tool::settings` (bounds `validate()` before any DB write).

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::common::AppError;

/// Sandbox modes admins may pick for unattended runs (Codex `SandboxMode`).
pub const VALID_SANDBOX_MODES: &[&str] =
    &["read-only", "workspace-write", "danger-full-access"];

/// Unattended-run approval policies (Codex `ApprovalMode`).
pub const VALID_APPROVAL_POLICIES: &[&str] =
    &["untrusted", "on-failure", "on-request", "never"];

/// Cap on the free-text `reviewer_policy` (matches the DB CHECK).
pub const MAX_REVIEWER_POLICY_LEN: usize = 32_768;

/// Deployment-wide agent policy (singleton row, `id = true`).
///
/// `reviewer_model_id` / `reviewer_policy` are intentionally nullable: NULL
/// means "fall back to the run's own model / the compiled-in reviewer
/// prompt" (zero-config). The token caps + step/fan-out limits are the
/// runtime-tunable knobs an operator adjusts per workload (DEC-6).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, sqlx::FromRow)]
pub struct AgentAdminSettings {
    pub default_sandbox_mode: String,
    pub unattended_approval_policy: String,
    pub reviewer_enabled: bool,
    pub reviewer_model_id: Option<Uuid>,
    pub reviewer_policy: Option<String>,
    pub reviewer_risk_thresholds: serde_json::Value,
    pub per_run_token_cap: i64,
    pub per_step_token_cap: i64,
    pub default_max_steps: i32,
    pub fan_out_max_threads: i32,
    pub fan_out_max_depth: i32,
    /// Max children accepted in ONE `delegate` call (DEC-1); over-cap truncates
    /// with an explicit "capped at N" note. Threaded into the crate's
    /// `SubagentLimits.max_children_per_call`.
    pub fan_out_max_children_per_call: i32,
    pub updated_at: DateTime<Utc>,
}

/// Partial-update request for the singleton. Every field optional (COALESCE
/// PATCH); the two nullable columns use the `Option<Option<T>>` tri-state:
///   missing  → `None`         → leave the column alone
///   `null`   → `Some(None)`   → clear the column back to its default
///   value    → `Some(Some(v))`→ set the column
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct UpdateAgentAdminSettingsRequest {
    pub default_sandbox_mode: Option<String>,
    pub unattended_approval_policy: Option<String>,
    pub reviewer_enabled: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_nullable_field")]
    pub reviewer_model_id: Option<Option<Uuid>>,
    #[serde(default, deserialize_with = "deserialize_nullable_field")]
    pub reviewer_policy: Option<Option<String>>,
    pub reviewer_risk_thresholds: Option<serde_json::Value>,
    pub per_run_token_cap: Option<i64>,
    pub per_step_token_cap: Option<i64>,
    pub default_max_steps: Option<i32>,
    pub fan_out_max_threads: Option<i32>,
    pub fan_out_max_depth: Option<i32>,
    pub fan_out_max_children_per_call: Option<i32>,
}

impl UpdateAgentAdminSettingsRequest {
    /// Validate ranges + enum membership before any DB write, so a bad value
    /// is a clean 400 instead of a raw 500 from the CHECK constraint. Bounds
    /// mirror the migration's CHECKs.
    pub fn validate(&self) -> Result<(), AppError> {
        fn bad(msg: impl Into<String>) -> AppError {
            AppError::bad_request("VALIDATION_ERROR", msg)
        }

        if let Some(m) = self.default_sandbox_mode.as_deref()
            && !VALID_SANDBOX_MODES.contains(&m)
        {
            return Err(bad(format!(
                "default_sandbox_mode must be one of {VALID_SANDBOX_MODES:?}"
            )));
        }
        if let Some(p) = self.unattended_approval_policy.as_deref()
            && !VALID_APPROVAL_POLICIES.contains(&p)
        {
            return Err(bad(format!(
                "unattended_approval_policy must be one of {VALID_APPROVAL_POLICIES:?}"
            )));
        }
        if let Some(Some(s)) = self.reviewer_policy.as_ref()
            && s.len() > MAX_REVIEWER_POLICY_LEN
        {
            return Err(bad("reviewer_policy exceeds the 32 KiB limit"));
        }
        if let Some(v) = self.reviewer_risk_thresholds.as_ref()
            && !v.is_object()
        {
            return Err(bad("reviewer_risk_thresholds must be a JSON object"));
        }
        if let Some(v) = self.per_run_token_cap
            && !(1_000..=1_000_000_000).contains(&v)
        {
            return Err(bad("per_run_token_cap out of range (1000..=1000000000)"));
        }
        if let Some(v) = self.per_step_token_cap
            && !(1_000..=1_000_000_000).contains(&v)
        {
            return Err(bad("per_step_token_cap out of range (1000..=1000000000)"));
        }
        if let Some(v) = self.default_max_steps
            && !(1..=1000).contains(&v)
        {
            return Err(bad("default_max_steps out of range (1..=1000)"));
        }
        if let Some(v) = self.fan_out_max_threads
            && !(1..=64).contains(&v)
        {
            return Err(bad("fan_out_max_threads out of range (1..=64)"));
        }
        if let Some(v) = self.fan_out_max_depth
            && !(1..=5).contains(&v)
        {
            return Err(bad("fan_out_max_depth out of range (1..=5)"));
        }
        if let Some(v) = self.fan_out_max_children_per_call
            && !(1..=64).contains(&v)
        {
            return Err(bad("fan_out_max_children_per_call out of range (1..=64)"));
        }
        Ok(())
    }
}

/// Distinguish "missing key" from "key present but null" so the PUT handler
/// treats null as "clear this column" and absent as "leave it alone." Local
/// copy matches `summarization::models` / `memory::models`.
fn deserialize_nullable_field<'de, D, T>(
    deserializer: D,
) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    Ok(Some(Option::<T>::deserialize(deserializer)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_patch_validates() {
        assert!(UpdateAgentAdminSettingsRequest::default().validate().is_ok());
    }

    #[test]
    fn rejects_bad_enum() {
        assert!(
            UpdateAgentAdminSettingsRequest {
                default_sandbox_mode: Some("nonsense".into()),
                ..Default::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            UpdateAgentAdminSettingsRequest {
                unattended_approval_policy: Some("nonsense".into()),
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn numeric_bounds() {
        assert!(
            UpdateAgentAdminSettingsRequest { default_max_steps: Some(0), ..Default::default() }
                .validate()
                .is_err()
        );
        assert!(
            UpdateAgentAdminSettingsRequest { default_max_steps: Some(30), ..Default::default() }
                .validate()
                .is_ok()
        );
        assert!(
            UpdateAgentAdminSettingsRequest { default_max_steps: Some(1001), ..Default::default() }
                .validate()
                .is_err()
        );
        assert!(
            UpdateAgentAdminSettingsRequest { fan_out_max_threads: Some(65), ..Default::default() }
                .validate()
                .is_err()
        );
        assert!(
            UpdateAgentAdminSettingsRequest {
                fan_out_max_children_per_call: Some(0),
                ..Default::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            UpdateAgentAdminSettingsRequest {
                fan_out_max_children_per_call: Some(65),
                ..Default::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            UpdateAgentAdminSettingsRequest {
                fan_out_max_children_per_call: Some(16),
                ..Default::default()
            }
            .validate()
            .is_ok()
        );
        assert!(
            UpdateAgentAdminSettingsRequest { per_run_token_cap: Some(999), ..Default::default() }
                .validate()
                .is_err()
        );
    }

    #[test]
    fn rejects_non_object_thresholds() {
        assert!(
            UpdateAgentAdminSettingsRequest {
                reviewer_risk_thresholds: Some(serde_json::json!("nope")),
                ..Default::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            UpdateAgentAdminSettingsRequest {
                reviewer_risk_thresholds: Some(serde_json::json!({"high": "prompt"})),
                ..Default::default()
            }
            .validate()
            .is_ok()
        );
    }
}
