//! Skill database row + create/update payloads.
//!
//! `Skill` mirrors the `skills` table verbatim (see migration
//! `00000000000095_create_skills_and_workflows_tables.sql`). Content
//! lives on disk under `extracted_path`; the row only carries metadata
//! + parsed SKILL.md frontmatter.

#![allow(dead_code)]

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Database row in `skills`. The bundle's SKILL.md + reference files
/// live on disk at `extracted_path`; the row carries metadata only.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, sqlx::FromRow)]
pub struct Skill {
    pub id: Uuid,
    /// Reverse-DNS canonical name (matches hub identity).
    pub name: String,
    /// Per-entry semver (the hub manifest's `version`).
    pub version: Option<String>,
    /// Display name from SKILL.md frontmatter `name`.
    pub display_name: Option<String>,
    /// SKILL.md frontmatter `description` — Path-B listing line the
    /// model sees in the system prompt.
    pub description: Option<String>,
    /// SKILL.md frontmatter `when_to_use` — supplemental trigger hint.
    pub when_to_use: Option<String>,
    /// Absolute path on disk to the extracted bundle dir.
    pub extracted_path: String,
    /// Hex-encoded sha256 of the original tar.gz; used for re-verification.
    pub bundle_sha256: String,
    pub bundle_size_bytes: i64,
    pub file_count: i32,
    /// Conventional entry-point file inside the bundle (`"SKILL.md"`).
    pub entry_point: String,
    /// FULL parsed YAML frontmatter — JSONB. Opaque-preserving so unknown
    /// fields (`allowed-tools`, `disable-model-invocation`, `paths`, etc.)
    /// round-trip if the bundle is re-exported.
    pub frontmatter_json: serde_json::Value,
    pub tags: serde_json::Value,
    pub scope: String, // 'user' | 'system'
    pub owner_user_id: Option<Uuid>,
    pub created_by: Option<Uuid>,
    pub enabled: bool,
    /// True when imported via `/api/skills/import` (dev workflow:
    /// mock: honored, no version bumping).
    pub is_dev: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Insert payload for `repository::insert`.
#[derive(Debug, Clone)]
pub struct CreateSkill {
    pub name: String,
    pub version: Option<String>,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub when_to_use: Option<String>,
    pub extracted_path: String,
    pub bundle_sha256: String,
    pub bundle_size_bytes: i64,
    pub file_count: i32,
    pub entry_point: String,
    pub frontmatter_json: serde_json::Value,
    pub tags: serde_json::Value,
    /// `'user'` or `'system'`.
    pub scope: String,
    /// REQUIRED when `scope == 'user'`; MUST be `None` when
    /// `scope == 'system'` (enforced by the table's CHECK).
    pub owner_user_id: Option<Uuid>,
    /// Audit-only — who issued the install (admin uid for system-scope
    /// installs, same as `owner_user_id` for user-scope).
    pub created_by: Option<Uuid>,
    pub enabled: bool,
    pub is_dev: bool,
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct UpdateSkill {
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub when_to_use: Option<String>,
    pub enabled: Option<bool>,
    pub tags: Option<serde_json::Value>,
}

/// Per-conversation OPT-OUT row. A row in
/// `conversation_skill_overrides` with `hidden=true` removes the skill
/// from the available-skills listing for that conversation only.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, sqlx::FromRow)]
pub struct ConversationSkillOverride {
    pub conversation_id: Uuid,
    pub skill_id: Uuid,
    pub hidden: bool,
    pub created_at: DateTime<Utc>,
}
