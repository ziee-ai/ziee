//! Workflow router.
//!
//! User-scope `/api/workflows/*` + `/api/workflow-runs/*` +
//! admin-scope `/api/workflows/system/*`. The canonical install paths
//! (`/api/workflows/install-from-hub`, `/api/workflows/system/install-from-hub`)
//! re-bind the hub install handlers (same as `skill/routes.rs`); the hub
//! module's `/api/hub/workflows/...` routes remain too.
//!
//! NOTE — no plain `POST /api/workflows` (or `/api/workflows/system`)
//! create endpoint: a hand-authored workflow bundle has no source of
//! truth without a file upload, so `import` (multipart tarball) IS the
//! create path. install-from-hub + import cover the real flows. This
//! mirrors the skills surface, which also omits a plain create. (An
//! intentional impl-differs vs. the plan's endpoint list — see plan §3.)

#![allow(dead_code)]

use aide::axum::{
    ApiRouter,
    routing::{delete_with, get_with, post_with, put_with},
};

use super::artifact_stream;
use super::elicit;
use super::handlers::{self, dev, system};
use super::log_stream;
use super::output_stream;
use super::progress_sse;

pub fn workflow_router() -> ApiRouter {
    ApiRouter::new()
        .merge(user_routes())
        .merge(admin_routes())
}

pub fn user_routes() -> ApiRouter {
    ApiRouter::new()
        // CRUD
        .api_route(
            "/workflows",
            get_with(handlers::list_user_workflows, handlers::list_user_workflows_docs),
        )
        .api_route(
            "/workflows/install-from-hub",
            // Thin re-bind of `hub::handlers::create_workflow_from_hub` at
            // the canonical user-facing path (mirrors skill/routes.rs).
            post_with(handlers::install_from_hub, handlers::install_from_hub_docs),
        )
        .api_route(
            "/workflows/{id}",
            get_with(handlers::get_user_workflow, handlers::get_user_workflow_docs),
        )
        .api_route(
            "/workflows/{id}",
            put_with(
                handlers::update_user_workflow,
                handlers::update_user_workflow_docs,
            ),
        )
        .api_route(
            "/workflows/{id}",
            delete_with(
                handlers::delete_user_workflow,
                handlers::delete_user_workflow_docs,
            ),
        )
        // Run
        .api_route(
            "/workflows/{id}/run",
            post_with(handlers::run_workflow, handlers::run_workflow_docs),
        )
        // Run history (A4)
        .api_route(
            "/workflows/{id}/runs",
            get_with(handlers::list_workflow_runs, handlers::list_workflow_runs_docs),
        )
        // Run lifecycle + read-back
        .api_route(
            "/workflow-runs/{run_id}",
            get_with(handlers::get_run, handlers::get_run_docs),
        )
        .api_route(
            "/workflow-runs/{run_id}/cancel",
            post_with(handlers::cancel_run, handlers::cancel_run_docs),
        )
        // Change an in-flight run's wall-clock timeout live (0 = unbounded)
        .api_route(
            "/workflow-runs/{run_id}/timeout",
            put_with(handlers::set_run_timeout, handlers::set_run_timeout_docs),
        )
        // Delete a terminal run (+ conditional artifact cascade) (A5)
        .api_route(
            "/workflow-runs/{run_id}",
            delete_with(handlers::delete_run, handlers::delete_run_docs),
        )
        .api_route(
            "/workflow-runs/{run_id}/events",
            get_with(progress_sse::subscribe, progress_sse::subscribe_docs),
        )
        .api_route(
            "/workflow-runs/{run_id}/output/{step_id}",
            get_with(output_stream::read_output, output_stream::read_output_docs),
        )
        .api_route(
            "/workflow-runs/{run_id}/artifact/{step_id}/{filename}",
            get_with(artifact_stream::read_artifact, artifact_stream::read_artifact_docs),
        )
        .api_route(
            "/workflow-runs/{run_id}/logs/{step_id}/{kind}",
            get_with(log_stream::read_log, log_stream::read_log_docs),
        )
        .api_route(
            "/workflow-runs/{run_id}/elicit/{elicitation_id}",
            post_with(elicit::submit_elicit, elicit::submit_elicit_docs),
        )
        // B6 dev/test surface
        .api_route(
            "/workflows/validate",
            post_with(dev::validate_workflow, dev::validate_workflow_docs),
        )
        .api_route(
            "/workflows/import",
            post_with(dev::import_workflow, dev::import_workflow_docs),
        )
        // Promote / download an LLM-authored bundle from the sandbox workspace.
        .api_route(
            "/workflows/workspace-save",
            post_with(dev::workspace_save, dev::workspace_save_docs),
        )
        .api_route(
            "/workflows/workspace-export",
            get_with(dev::workspace_export, dev::workspace_export_docs),
        )
        .api_route(
            "/workflows/{id}/dry-run",
            post_with(dev::dry_run, dev::dry_run_docs),
        )
        .api_route(
            "/workflows/{id}/test",
            post_with(dev::test_workflow, dev::test_workflow_docs),
        )
}

pub fn admin_routes() -> ApiRouter {
    ApiRouter::new()
        .api_route(
            "/workflows/system",
            get_with(system::list_system_workflows, system::list_system_workflows_docs),
        )
        .api_route(
            "/workflows/system/install-from-hub",
            post_with(
                handlers::install_system_from_hub,
                handlers::install_system_from_hub_docs,
            ),
        )
        .api_route(
            "/workflows/system/import",
            post_with(dev::import_system_workflow, dev::import_system_workflow_docs),
        )
        .api_route(
            "/workflows/system/{id}",
            get_with(system::get_system_workflow, system::get_system_workflow_docs),
        )
        .api_route(
            "/workflows/system/{id}",
            delete_with(
                system::delete_system_workflow,
                system::delete_system_workflow_docs,
            ),
        )
        .api_route(
            "/workflows/system/{id}/groups",
            get_with(system::get_workflow_groups, system::get_workflow_groups_docs),
        )
        .api_route(
            "/workflows/system/{id}/groups",
            post_with(system::set_workflow_groups, system::set_workflow_groups_docs),
        )
        .api_route(
            "/workflows/system/{id}/groups/{group_id}",
            delete_with(
                system::remove_workflow_group,
                system::remove_workflow_group_docs,
            ),
        )
        // Group-centric assignment (User Groups page widget)
        .api_route(
            "/groups/{group_id}/system-workflows",
            get_with(
                system::get_group_system_workflows,
                system::get_group_system_workflows_docs,
            ),
        )
        .api_route(
            "/groups/{group_id}/system-workflows",
            put_with(
                system::update_group_system_workflows,
                system::update_group_system_workflows_docs,
            ),
        )
}
