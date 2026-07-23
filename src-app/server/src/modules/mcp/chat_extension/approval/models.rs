//! MCP approval workflow models

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::modules::mcp::chat_extension::defaults::models::LoopSettings;

// ── The `#[default]` variant below is THE deployment default ────────────────────
//
// NOTE: this block is a plain comment, NOT a doc comment, on purpose — doc comments
// on an OpenAPI-exposed type flow through to `ui/src/api-client/types.ts` as JSDoc,
// and this is internal backend guidance, not client-facing API documentation.
//
// `ApprovalMode::default()` is the SINGLE SOURCE OF TRUTH for "what approval mode
// applies when nothing has been stored yet". Every no-row / unparseable fallback in
// the MCP module derives from it:
//
//   - `ConversationMcpSettings::get_approval_mode` / `UserMcpDefaults::get_approval_mode`
//     (`.unwrap_or_default()`)
//   - the no-settings-and-no-user-defaults branch of `mcp.rs::resolve_approval`
//   - `mcp/settings/repository.rs::default_approval_mode()`, which feeds
//     `get_or_default` and the settings upsert
//   - the insert-side `COALESCE` in `approval/repository.rs::upsert_conversation_settings`
//     and `defaults/repository.rs::upsert_user_defaults`
//
// It is therefore the ONE line that intentionally differs between branches:
// `ManualApprove` on `main`/`khoi`, `AutoApprove` on `deploy-schedule` (where the
// deployment runs its org MCP tools without a per-call prompt). Adding a new
// fallback? Route it through here — never spell a mode inline. Five disagreeing
// copies of this value are exactly why a conversation could auto-approve on turn 1
// and then prompt on turn 2.
//
// The DB column default (`202607140180_mcp_schema.sql:56,132`) is deliberately NOT
// kept in sync: it is unreachable, because every INSERT in the tree names
// `approval_mode` explicitly.
/// Approval mode for conversation MCP settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ApprovalMode {
    /// MCP is disabled for this conversation
    Disabled,
    /// All tools are auto-approved
    //
    // ── THE deploy default. This `#[default]` attribute is the ONE intentional
    // divergence from main/khoi (which mark ManualApprove instead). It is the
    // single line to resolve when porting an mcp-approval change across the two
    // branches — everything else derives from it (see the block above).
    //
    // This deployment runs its org MCP tools without a per-call prompt.
    #[default]
    AutoApprove,
    /// Manual approval required for each tool use
    ManualApprove,
}


impl std::fmt::Display for ApprovalMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApprovalMode::Disabled => write!(f, "disabled"),
            ApprovalMode::AutoApprove => write!(f, "auto_approve"),
            ApprovalMode::ManualApprove => write!(f, "manual_approve"),
        }
    }
}

impl std::str::FromStr for ApprovalMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "disabled" => Ok(ApprovalMode::Disabled),
            "auto_approve" => Ok(ApprovalMode::AutoApprove),
            "manual_approve" => Ok(ApprovalMode::ManualApprove),
            _ => Err(format!("Invalid approval mode: {}", s)),
        }
    }
}

/// Auto-approved tools grouped by server
/// Format: [{"server_id": "uuid", "tools": ["tool1", "tool2"]}, ...]
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AutoApprovedServer {
    /// MCP server ID (UUID)
    pub server_id: Uuid,
    /// List of tool names that are auto-approved for this server
    pub tools: Vec<String>,
}

impl AutoApprovedServer {
    /// Check if a specific tool is auto-approved for this server
    pub fn contains_tool(&self, tool_name: &str) -> bool {
        self.tools.iter().any(|t| t == tool_name)
    }
}

/// Disabled servers/tools for a conversation
/// Format: [{"server_id": "uuid", "tools": []}, ...]
/// Empty tools array = entire server disabled
/// Non-empty tools array = only those specific tools disabled
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DisabledServer {
    /// MCP server ID (UUID)
    pub server_id: Uuid,
    /// List of disabled tool names (empty = entire server disabled)
    pub tools: Vec<String>,
}

#[allow(dead_code)]
impl DisabledServer {
    /// Check if entire server is disabled (empty tools = all disabled)
    pub fn is_server_disabled(&self) -> bool {
        self.tools.is_empty()
    }

    /// Check if a specific tool is disabled for this server
    pub fn is_tool_disabled(&self, tool_name: &str) -> bool {
        // If tools is empty, entire server is disabled
        if self.tools.is_empty() {
            return true;
        }
        self.tools.iter().any(|t| t == tool_name)
    }
}

/// Conversation MCP settings (database model)
#[derive(Debug, Clone, Deserialize, FromRow)]
pub struct ConversationMcpSettings {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub user_id: Uuid,

    /// Approval mode (stored as VARCHAR in DB)
    pub approval_mode: String,

    /// Auto-approved tools (JSON array stored in DB)
    pub auto_approved_tools: serde_json::Value,

    /// Disabled servers/tools (JSON array stored in DB)
    pub disabled_servers: serde_json::Value,

    /// Loop settings (JSON object stored in DB)
    pub loop_settings: Option<serde_json::Value>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ConversationMcpSettings {
    /// Get approval mode as enum.
    ///
    /// A stored value that doesn't parse falls back to [`ApprovalMode::default()`]
    /// (the deployment default) rather than a hardcoded variant — see the type-level
    /// docs on [`ApprovalMode`]. In practice unreachable: every writer goes through
    /// `ApprovalMode::to_string()`, so the column only ever holds one of the three
    /// known spellings.
    pub fn get_approval_mode(&self) -> ApprovalMode {
        self.approval_mode.parse().unwrap_or_default()
    }

    /// Get auto-approved tools as typed Vec
    pub fn get_auto_approved_tools(&self) -> Vec<AutoApprovedServer> {
        serde_json::from_value(self.auto_approved_tools.clone()).unwrap_or_default()
    }

    /// Get disabled servers as typed Vec
    pub fn get_disabled_servers(&self) -> Vec<DisabledServer> {
        serde_json::from_value(self.disabled_servers.clone()).unwrap_or_default()
    }

    /// Get loop settings
    pub fn get_loop_settings(&self) -> LoopSettings {
        self.loop_settings
            .as_ref()
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default()
    }
}

/// Conversation MCP settings (API response - properly typed)
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct ConversationMcpSettingsResponse {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub user_id: Uuid,

    /// Approval mode
    pub approval_mode: ApprovalMode,

    /// Auto-approved tools grouped by server
    pub auto_approved_tools: Vec<AutoApprovedServer>,

    /// Disabled servers/tools (empty = all servers enabled)
    pub disabled_servers: Vec<DisabledServer>,

    /// Loop settings for controlling iteration behavior
    pub loop_settings: LoopSettings,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<ConversationMcpSettings> for ConversationMcpSettingsResponse {
    fn from(settings: ConversationMcpSettings) -> Self {
        Self {
            id: settings.id,
            conversation_id: settings.conversation_id,
            user_id: settings.user_id,
            approval_mode: settings.get_approval_mode(),
            auto_approved_tools: settings.get_auto_approved_tools(),
            disabled_servers: settings.get_disabled_servers(),
            loop_settings: settings.get_loop_settings(),
            created_at: settings.created_at,
            updated_at: settings.updated_at,
        }
    }
}

/// Tool use approval record
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, schemars::JsonSchema)]
pub struct ToolUseApproval {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub branch_id: Uuid,
    pub message_id: Uuid,
    pub user_id: Uuid,

    pub tool_use_id: String,
    pub tool_name: String,

    /// Tool input (serialized as "input" for API compatibility)
    #[serde(rename = "input")]
    pub tool_input: serde_json::Value,

    /// Server identification (hybrid approach)
    pub server_id: Option<Uuid>,
    pub server_name: String,

    /// Approval status (stored as VARCHAR in DB, serialized as String for API)
    pub status: String, // Stored as VARCHAR, converted to/from ApprovalStatus

    pub approved_at: Option<DateTime<Utc>>,
    pub approved_by: Option<Uuid>,
    pub approval_note: Option<String>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request to create/update MCP settings
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct UpsertMcpSettingsRequest {
    // Omitting this is what the client's initial per-conversation auto-persist does:
    // that write exists to snapshot `disabled_servers`, and must not silently pin an
    // approval mode the user never chose (the "auto-approved on turn 1, prompts on
    // turn 2" bug). Same tri-state contract as `auto_approved_tools` below.
    //
    // `#[serde(default)]` on an `Option` yields `None`, NOT
    // `Some(ApprovalMode::default())` — absent stays distinguishable from explicit,
    // which is what the repository's `COALESCE` needs.
    /// Approval mode. Omit to leave it alone: an existing setting is preserved, and a
    /// scope with no stored settings gets the server's default.
    #[serde(default)]
    pub approval_mode: Option<ApprovalMode>,

    /// Auto-approved tools grouped by server
    /// Format: [{"server_id": "uuid", "tools": ["tool1", "tool2"]}, ...]
    /// None = preserve existing value in DB; Some(vec) = overwrite with this value
    #[serde(default)]
    pub auto_approved_tools: Option<Vec<AutoApprovedServer>>,

    /// Disabled servers/tools (empty = all servers enabled)
    /// Format: [{"server_id": "uuid", "tools": []}, ...] (empty tools = entire server disabled)
    #[serde(default)]
    pub disabled_servers: Vec<DisabledServer>,

    /// Loop settings for controlling iteration behavior
    #[serde(default)]
    pub loop_settings: LoopSettings,
}

/// Single tool approval decision
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ToolApprovalDecision {
    /// Tool use ID
    pub tool_use_id: String,

    /// Decision: "approve" | "deny"
    pub decision: String,

    /// Optional note
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings_with_mode(raw: &str) -> ConversationMcpSettings {
        ConversationMcpSettings {
            id: Uuid::nil(),
            conversation_id: Uuid::nil(),
            user_id: Uuid::nil(),
            approval_mode: raw.to_string(),
            auto_approved_tools: serde_json::json!([]),
            disabled_servers: serde_json::json!([]),
            loop_settings: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// TEST-1 — a stored mode parses back to itself, and anything unparseable falls
    /// back to the branch's compiled default rather than a hardcoded variant.
    #[test]
    fn conversation_get_approval_mode_parses_or_defaults() {
        assert_eq!(
            settings_with_mode("disabled").get_approval_mode(),
            ApprovalMode::Disabled
        );
        assert_eq!(
            settings_with_mode("auto_approve").get_approval_mode(),
            ApprovalMode::AutoApprove
        );
        assert_eq!(
            settings_with_mode("manual_approve").get_approval_mode(),
            ApprovalMode::ManualApprove
        );

        // Asserted against ApprovalMode::default(), NOT a literal — this same test
        // must hold on deploy-schedule, where the default is AutoApprove.
        for junk in ["", "AUTO_APPROVE", "auto-approve", "nonsense"] {
            assert_eq!(
                settings_with_mode(junk).get_approval_mode(),
                ApprovalMode::default(),
                "unparseable {junk:?} must fall back to the deployment default",
            );
        }
    }

    /// TEST-3 — the enum default and its DB string spelling can never drift apart.
    /// `settings/repository.rs::default_approval_mode()` and both upserts' insert-side
    /// COALESCE stringify the default and store it; the read path parses it back. If
    /// `Display` and `FromStr` disagreed for the default variant, a freshly-inserted
    /// row would read back as a DIFFERENT mode than the one just written.
    #[test]
    fn default_approval_mode_round_trips_through_its_db_spelling() {
        let stored = ApprovalMode::default().to_string();
        assert_eq!(
            stored.parse::<ApprovalMode>().expect("default must parse"),
            ApprovalMode::default(),
        );
        // And the round-trip holds via the real read path, not just FromStr.
        assert_eq!(
            settings_with_mode(&stored).get_approval_mode(),
            ApprovalMode::default(),
        );
    }

    /// TEST-6 — absent must stay distinguishable from explicit. This is the property
    /// the repository COALESCE relies on: `#[serde(default)]` on an `Option` yields
    /// `None`, not `Some(ApprovalMode::default())`. If this ever regressed to the
    /// latter, an un-customized client save would start pinning a mode again and the
    /// original bug would return.
    #[test]
    fn upsert_request_distinguishes_absent_from_explicit_approval_mode() {
        let absent: UpsertMcpSettingsRequest =
            serde_json::from_value(serde_json::json!({ "disabled_servers": [] }))
                .expect("approval_mode must be optional");
        assert_eq!(absent.approval_mode, None);

        for (raw, expected) in [
            ("disabled", ApprovalMode::Disabled),
            ("auto_approve", ApprovalMode::AutoApprove),
            ("manual_approve", ApprovalMode::ManualApprove),
        ] {
            let explicit: UpsertMcpSettingsRequest = serde_json::from_value(
                serde_json::json!({ "approval_mode": raw, "disabled_servers": [] }),
            )
            .expect("explicit approval_mode must deserialize");
            assert_eq!(explicit.approval_mode, Some(expected));
        }
    }
}
