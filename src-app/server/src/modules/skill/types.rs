//! Request / response DTOs for the skill REST surface.
//!
//! Phase B6 fleshes out the full CRUD + visibility-query DTOs. The
//! create/list types defined here are the minimum to compile the
//! install handlers (B2) + the chat extension + skill_mcp (B3).

#![allow(dead_code)]

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::models::Skill;

/// `POST /api/skills/install-from-hub` body. Mirrors
/// `CreateAssistantFromHubRequest` — just the hub identity, server
/// derives the rest from the bundle.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct CreateSkillFromHubRequest {
    /// Hub skill ID (reverse-DNS canonical name).
    pub hub_id: String,
}

/// `POST /api/skills/system/install-from-hub` body. Same as the
/// user-scope variant plus optional group assignment.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct CreateSystemSkillFromHubRequest {
    pub hub_id: String,
    /// Optional list of group IDs to assign in the same TX as the install.
    #[serde(default)]
    pub groups: Vec<Uuid>,
}

/// Response from any install-from-hub endpoint.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SkillFromHubResponse {
    pub skill: Skill,
    pub hub_tracking: crate::modules::hub::models::HubEntity,
}

/// `GET /api/skills` response shape — user-owned + accessible system
/// skills, each tagged with its `scope`.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SkillListResponse {
    pub skills: Vec<Skill>,
}

/// Lightweight `GET /api/skills/available` entry — what the chat
/// extension AND `skill_mcp::list_tools` consume. Mirrors
/// `repository::SkillAvailableEntry` but with JsonSchema for the REST
/// surface.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct AvailableSkillEntry {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub when_to_use: Option<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct AvailableSkillsResponse {
    pub skills: Vec<AvailableSkillEntry>,
}

/// `GET /api/skills/{id}/body` response — the SKILL.md markdown body
/// (frontmatter stripped), read from the extracted bundle on disk. Lets
/// the FE SkillDetailDrawer render the real procedural content, not just
/// the frontmatter-derived metadata (plan §5 "renders SKILL.md").
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SkillBodyResponse {
    pub body: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct AvailableSkillsQuery {
    pub conversation_id: Uuid,
}

/// `POST /api/skills/{id}/hide-in-conversation` body.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct HideSkillInConversationRequest {
    pub conversation_id: Uuid,
}

/// `POST /api/skills/system/{id}/groups` body. Replaces the entire set
/// (mirrors `mcp/handlers/groups.rs`'s `ServerGroupsRequest`).
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct SkillGroupsRequest {
    pub group_ids: Vec<Uuid>,
}

/// `GET/PUT /api/groups/{group_id}/system-skills` response — the system
/// skills assigned to a group (group → skills direction, for the User
/// Groups page widget). Mirrors MCP's `GroupSystemServersResponse`.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct GroupSystemSkillsResponse {
    pub skills: Vec<Skill>,
}

/// `PUT /api/groups/{group_id}/system-skills` body — the full desired set
/// of system-skill ids for the group. Mirrors MCP's
/// `UpdateGroupSystemServersRequest`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct UpdateGroupSystemSkillsRequest {
    pub skill_ids: Vec<Uuid>,
}
