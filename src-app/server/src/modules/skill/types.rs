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

/// `GET /api/skills` response shape — TODO B6 wire the visibility-query
/// union. For now, the install handlers don't need a list type.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SkillListResponse {
    pub skills: Vec<Skill>,
}
