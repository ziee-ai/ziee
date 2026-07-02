//! Summarization module data types — DTOs + DB row types.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Deployment-wide summarization admin settings (singleton row).
///
/// `default_summarization_model_id` is intentionally nullable: when
/// NULL, the chat extension falls back to the conversation's own
/// model (zero-config). The token thresholds + prompt overrides are
/// runtime-tunable knobs for operators with workloads that need a
/// different shape than the compiled defaults.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, sqlx::FromRow)]
pub struct SummarizationAdminSettings {
    pub id: i16,
    pub enabled: bool,
    pub default_summarization_model_id: Option<Uuid>,
    pub summarize_after_tokens: i32,
    pub summarizer_keep_recent_tokens: i32,
    /// Custom prompt for the full-resume path. NULL → use the compiled-in
    /// default. Must contain `{transcript}` placeholder when set.
    pub full_summary_prompt: Option<String>,
    /// Custom prompt for the incremental-fold path. NULL → use the
    /// compiled-in default. Must contain `{previous_summary}` AND
    /// `{new_transcript}` placeholders when set.
    pub incremental_summary_prompt: Option<String>,
    pub updated_at: DateTime<Utc>,
}

/// Partial-update request for the singleton admin settings row.
///
/// The four nullable fields (model id + the two prompts) use the
/// `Option<Option<T>>` tri-state:
///   missing  → `None`        → leave the column alone
///   `null`   → `Some(None)`  → clear the column
///   value    → `Some(Some(v))` → set the column
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct UpdateSummarizationAdminSettingsRequest {
    pub enabled: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_nullable_field")]
    pub default_summarization_model_id: Option<Option<Uuid>>,
    pub summarize_after_tokens: Option<i32>,
    pub summarizer_keep_recent_tokens: Option<i32>,
    #[serde(default, deserialize_with = "deserialize_nullable_field")]
    pub full_summary_prompt: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable_field")]
    pub incremental_summary_prompt: Option<Option<String>>,
}

/// Response for `GET /api/conversations/{id}/summarization-mode`.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ConversationSummarizationModeResponse {
    pub summarization_mode: String,
}

/// Request body for `PUT /api/conversations/{id}/summarization-mode`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct UpdateConversationSummarizationModeRequest {
    pub summarization_mode: String,
}

pub const VALID_SUMMARIZATION_MODES: &[&str] = &["inherit", "on", "off"];

pub fn is_valid_summarization_mode(mode: &str) -> bool {
    VALID_SUMMARIZATION_MODES.contains(&mode)
}

/// Distinguish "missing key in the JSON" from "key present but null".
/// Lets the PUT handler treat null as "clear this column" and absent as
/// "leave it alone." Local copy matches the pattern used in
/// `memory::models` and `chat::core::types`.
fn deserialize_nullable_field<'de, D, T>(
    deserializer: D,
) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    Ok(Some(Option::<T>::deserialize(deserializer)?))
}
