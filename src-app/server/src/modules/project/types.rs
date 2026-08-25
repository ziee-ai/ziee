// Project API request/response types.
// Separated from models.rs so the DB entity stays clean.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::common::AppError;

use super::models::Project;

/// Request to create a project.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateProjectRequest {
    #[serde(default)]
    #[schemars(length(min = 1, max = 255))]
    pub name: String,

    /// Brief description. Capped at 4 KiB (same as assistant) to avoid
    /// per-turn token-cost amplification when project context is
    /// injected.
    #[schemars(length(max = 4096))]
    pub description: Option<String>,

    /// System instructions injected into every conversation under this
    /// project. Capped at 64 KiB (same as assistant).
    #[schemars(length(max = 65_536))]
    pub instructions: Option<String>,

    pub default_assistant_id: Option<Uuid>,
    pub default_model_id: Option<Uuid>,
}

/// Request to update an existing project. All fields optional.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UpdateProjectRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 255))]
    pub name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(length(max = 4096))]
    pub description: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(length(max = 65_536))]
    pub instructions: Option<String>,

    /// Tri-state on FKs (missing = no change; null = clear; uuid = set).
    /// The frontend uses the existing deserialize_nullable_field helper
    /// from the chat module for symmetry.
    #[serde(
        default,
        deserialize_with = "deserialize_nullable_field",
        skip_serializing_if = "Option::is_none"
    )]
    pub default_assistant_id: Option<Option<Uuid>>,
    #[serde(
        default,
        deserialize_with = "deserialize_nullable_field",
        skip_serializing_if = "Option::is_none"
    )]
    pub default_model_id: Option<Option<Uuid>>,
}

/// Tri-state field deserializer: a missing key yields `None` (no change)
/// while an explicit JSON `null` yields `Some(None)` (clear). Without this
/// custom deserializer serde collapses both cases to `None`, so the
/// "clear" arm is unreachable. Mirrors
/// `chat::core::types::deserialize_nullable_field`.
fn deserialize_nullable_field<'de, D, T>(
    deserializer: D,
) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    Ok(Some(Option::<T>::deserialize(deserializer)?))
}

/// List response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProjectListResponse {
    pub projects: Vec<Project>,
    pub total: i64,
}

/// Hard cap on how many conversation ids one batch reverse-lookup may carry.
///
/// This is a request-VALIDATION bound (a payload guard on a single read), not
/// an operational tunable — same class as the 100-files-per-project cap, and
/// like it, over-cap answers **422**. Declared as a named constant rather than
/// an inline literal so it can be promoted to a settings row later without a
/// rewrite. The client chunks its batches at this size.
pub const MAX_CONVERSATIONS_PER_LOOKUP: usize = 200;

/// Request body for `POST /api/projects/by-conversations` — the BATCH form of
/// `GET /api/projects/by-conversation/{id}`.
///
/// Exists because a conversation list renders one membership badge per row: the
/// per-id endpoint turned a 40-row sidebar into 40 requests (an `n+1` burst the
/// live-ui-audit flags). Duplicate ids are tolerated (de-duplicated server-side).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProjectsByConversationsRequest {
    /// The conversations to resolve. At most `MAX_CONVERSATIONS_PER_LOOKUP`.
    pub conversation_ids: Vec<Uuid>,
}

impl ProjectsByConversationsRequest {
    /// Reject an over-cap batch with a 422 before touching the database.
    ///
    /// Pure + total so it is directly unit-testable (no pool, no auth). Mirrors
    /// `UpdateJsToolSettings::validate` (js_tool/settings.rs) — a request type
    /// owning its own bounds check.
    pub fn validate(&self) -> Result<(), AppError> {
        if self.conversation_ids.len() > MAX_CONVERSATIONS_PER_LOOKUP {
            return Err(AppError::unprocessable_entity(
                "TOO_MANY_CONVERSATION_IDS",
                format!(
                    "Request at most {max} conversations per lookup; split the list into \
                     batches of {max} or fewer (received {got}).",
                    max = MAX_CONVERSATIONS_PER_LOOKUP,
                    got = self.conversation_ids.len()
                ),
            ));
        }
        Ok(())
    }
}

/// One resolved membership: `conversation_id` is attached to `project_id`.
///
/// Carries the project ID rather than the project ROW: a conversation list is
/// typically many conversations across FEW projects, so inlining the row per
/// link would repeat the same (up-to-64-KiB `instructions`) project up to 200
/// times in one response. The rows are de-duplicated into
/// [`ProjectsByConversationsResponse::projects`]; join on `project_id`.
///
/// Conversations that are unfiled — or that belong to another user, or don't
/// exist — are simply ABSENT from `links` (never a null entry), mirroring the
/// singular endpoint's `null` answer and its "unfiled is legitimate data, not
/// an error" contract.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConversationProjectLink {
    pub conversation_id: Uuid,
    pub project_id: Uuid,
}

/// Response body for `POST /api/projects/by-conversations`.
///
/// `links` maps each ATTACHED conversation to a project id; `projects` lists
/// each referenced project EXACTLY ONCE (the same shape the singular endpoint
/// returns), so the payload is O(conversations + projects), not
/// O(conversations x project size).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProjectsByConversationsResponse {
    pub links: Vec<ConversationProjectLink>,
    pub projects: Vec<Project>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(n: usize) -> ProjectsByConversationsRequest {
        ProjectsByConversationsRequest {
            conversation_ids: (0..n).map(|_| Uuid::new_v4()).collect(),
        }
    }

    #[test]
    fn empty_batch_is_valid() {
        assert!(req(0).validate().is_ok());
    }

    #[test]
    fn batch_at_the_cap_is_valid() {
        assert!(req(MAX_CONVERSATIONS_PER_LOOKUP).validate().is_ok());
    }

    #[test]
    fn over_cap_batch_is_rejected_as_422() {
        let err = req(MAX_CONVERSATIONS_PER_LOOKUP + 1)
            .validate()
            .expect_err("over-cap batch must be rejected");
        assert_eq!(err.status_code(), 422);
        assert_eq!(err.error_code(), "TOO_MANY_CONVERSATION_IDS");
    }
}

// `AttachFileRequest` + `ProjectFileListResponse` moved to
// `modules/file/project_extension/models.rs` as part of the project↔file
// inversion. The four `/api/projects/{id}/files*` routes that consume
// them are now owned by the file module.
