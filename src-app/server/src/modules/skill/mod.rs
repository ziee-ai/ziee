//! Skill module — Agent Skills standard bundles (per plan §3 + §4.7).
//!
//! Phase B2 ships the SKELETONS only: models / repository (insert /
//! find_by_name_version / delete) / permissions / events / SKILL.md
//! frontmatter parser. Enough for the hub install handlers
//! (`hub::handlers::create_skill_from_hub` +
//! `create_system_skill_from_hub`) to compile.
//!
//! B3 fills out:
//! - The chat extension at order 15 (Path B listing-only injection).
//! - The visibility-query union (user-owned + accessible-system minus
//!   per-conversation hides) backing both the chat extension AND
//!   `skill_mcp::list_tools`.
//! - The built-in `skill_mcp` MCP server exposing `load_skill` +
//!   `read_skill_file` tools.
//! - The full user / system CRUD REST surface
//!   (mirrors `mcp/handlers/system.rs`).

pub mod builtin;
pub mod chat_extension;
pub mod dev_handlers;
pub mod events;
pub mod frontmatter;
pub mod handlers;
pub mod models;
pub mod permissions;
pub mod repository;
pub mod routes;
pub mod types;

pub use repository::SkillRepository;

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use std::error::Error;

use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};

#[distributed_slice(MODULE_ENTRIES)]
static SKILL_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "skill",
    // After users / hub (so hub_entities exists) and before chat (so
    // the chat extension can self-register in B3).
    order: 81,
    description: "Agent Skills bundles + per-conversation hide overrides",
    constructor: || Box::new(SkillModule::new()),
};

pub struct SkillModule;

impl SkillModule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SkillModule {
    fn default() -> Self {
        Self::new()
    }
}

impl AppModule for SkillModule {
    fn name(&self) -> &'static str {
        "skill"
    }

    fn description(&self) -> &'static str {
        "Agent Skills bundles"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        // Sync ziee's embedded built-in capability skills into the DB as
        // scope='built_in' rows (idempotent upsert). Spawned (init is sync)
        // — mirrors skill_mcp's built-in server registration. The 10-row
        // upsert completes in ms; built-ins are version-locked to the binary.
        let pool = (*ctx.db_pool).clone();
        tokio::spawn(async move {
            match builtin::sync_builtin_skills(&pool).await {
                Ok(n) => tracing::info!("skill: synced {n} built-in capability skill(s)"),
                Err(e) => tracing::warn!(error = %e, "skill: built-in sync failed"),
            }
        });
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router
            .merge(routes::user_routes())
            .merge(routes::admin_routes())
    }
}
