//! Workflow module — declarative DAG runner with `llm` / `llm_map` /
//! `sandbox` / `elicit` step kinds (see plan §4.0, §4.2, §4.5).
//!
//! Phase B2 ships the SKELETON only: models / repository (workflows +
//! workflow_runs insert/find/mark_status) / permissions / events
//! stubs / workflow.yaml parser stub (deserialization + cycle check).
//!
//! B4 ships the runner — dispatcher trait + the four step impls, the
//! template engine, the per-run SSE channel, model snapshotting,
//! cancellation, the startup orphan-run sweep, the staging
//! contract (`stage_workspace_subdir` integration with code_sandbox).
//!
//! B6 ships the REST surface (user + admin + workflow-runs handlers)
//! per plan §3.

pub mod events;
pub mod handlers;
pub mod models;
pub mod permissions;
pub mod repository;
pub mod routes;
pub mod types;
pub mod validate;

pub use repository::WorkflowRepository;

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use std::error::Error;

use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};

#[distributed_slice(MODULE_ENTRIES)]
static WORKFLOW_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "workflow",
    // After skill (81) + code_sandbox (70) + hub. Workflow runner
    // depends on code_sandbox for `kind: sandbox` step staging
    // (`stage_workspace_subdir`) and on hub_entities for tracking
    // installed workflows.
    order: 82,
    description: "Declarative DAG runner (llm / llm_map / sandbox / elicit steps)",
    constructor: || Box::new(WorkflowModule::new()),
};

pub struct WorkflowModule;

impl WorkflowModule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WorkflowModule {
    fn default() -> Self {
        Self::new()
    }
}

impl AppModule for WorkflowModule {
    fn name(&self) -> &'static str {
        "workflow"
    }

    fn description(&self) -> &'static str {
        "Workflow DAG runner"
    }

    fn init(&mut self, _ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        // B4: spawn the startup orphan-run sweep here (any
        // `workflow_runs` row left in `pending` / `running` from a
        // crash before this boot gets marked Failed). Also wires the
        // per-run registry singleton.
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(routes::workflow_router())
    }
}
