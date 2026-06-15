//! Workflow router (B4 surface).
//!
//! User-scope `/api/workflows/*` + `/api/workflow-runs/*` +
//! admin-scope `/api/workflows/system/*`. The hub install
//! (`/api/hub/workflows/...`) stays in the hub module.

#![allow(dead_code)]

use aide::axum::{
    ApiRouter,
    routing::{delete_with, get_with, post_with},
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
            "/workflows/{id}",
            get_with(handlers::get_user_workflow, handlers::get_user_workflow_docs),
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
        // Run lifecycle + read-back
        .api_route(
            "/workflow-runs/{run_id}",
            get_with(handlers::get_run, handlers::get_run_docs),
        )
        .api_route(
            "/workflow-runs/{run_id}/cancel",
            post_with(handlers::cancel_run, handlers::cancel_run_docs),
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
}
