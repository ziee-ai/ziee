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

pub mod artifact_io;
pub mod artifact_stream;
pub mod compiled;
pub mod cost;
pub mod dispatch;
pub mod elicit;
pub mod events;
pub mod file_io;
pub mod handlers;
pub mod log_io;
pub mod log_stream;
pub mod models;
pub mod output_stream;
pub mod permissions;
pub mod progress_sse;
pub mod ref_check;
pub mod registry;
pub mod repository;
pub mod routes;
pub mod runner;
pub mod startup_sweep;
pub mod template;
pub mod test_runner;
pub mod type_infer;
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

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        // Startup sweep: flip every orphan in-flight workflow_run row
        // to `failed` ("server restart during execution") and remove
        // stale staged dirs under <workspace_root>/*/workflow/*/.
        let pool = (*ctx.db_pool).clone();
        tokio::spawn(async move {
            if let Err(e) = startup_sweep::sweep_at_boot(&pool).await {
                tracing::warn!(error = %e, "workflow: startup sweep failed");
            }
        });
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(routes::workflow_router())
    }
}
