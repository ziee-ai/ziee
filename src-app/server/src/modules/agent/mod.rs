//! Agent module — deployment-wide agent policy (ITEM-28 / DEC-6).
//!
//! Server-side singleton settings surface for the shared `agent-core` loop:
//!   - REST admin: `GET/PUT /api/agent/settings` for the deployment-wide
//!     `agent_admin_settings` singleton row (sandbox/approval mode, reviewer
//!     config, token caps, max steps, fan-out guardrails).
//!
//! Model + repo + read-at-use are SERVER-side (the `agent-core` crate stays
//! domain/DB-free). Mirrors `summarization` / `js_tool` singleton-settings
//! modules. Admin-only (`agent::settings::{read,manage}`, `*`-wildcard —
//! no grant migration).

pub mod handlers;
pub mod models;
pub mod permissions;
pub mod repository;
pub mod routes;

pub use repository::AgentRepository;

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use std::error::Error;

use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};

#[distributed_slice(MODULE_ENTRIES)]
static AGENT_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "agent",
    // After llm_model (reviewer_model_id references a model). Same ordering
    // tier as memory/summarization.
    order: 80,
    description: "Deployment-wide agent policy settings",
    constructor: || Box::new(AgentModule::new()),
};

pub struct AgentModule;

impl AgentModule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AgentModule {
    fn default() -> Self {
        Self::new()
    }
}

impl AppModule for AgentModule {
    fn name(&self) -> &'static str {
        "agent"
    }

    fn description(&self) -> &'static str {
        "Deployment-wide agent policy settings"
    }

    fn init(&mut self, _ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(routes::agent_router())
    }
}
