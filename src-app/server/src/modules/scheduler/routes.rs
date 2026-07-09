//! REST surface for scheduled-task CRUD + the admin-settings singleton.
//! (run-now / test-fire land once the dispatch seam exists.)

use aide::axum::{
    ApiRouter,
    routing::get_with,
};

use super::handlers;

pub fn scheduler_router() -> ApiRouter {
    ApiRouter::new()
        .api_route(
            "/scheduled-tasks",
            get_with(handlers::list_tasks, handlers::list_tasks_docs)
                .post_with(handlers::create_task, handlers::create_task_docs),
        )
        .api_route(
            "/scheduled-tasks/{id}",
            get_with(handlers::get_task, handlers::get_task_docs)
                .put_with(handlers::update_task, handlers::update_task_docs)
                .delete_with(handlers::delete_task, handlers::delete_task_docs),
        )
        .api_route(
            "/scheduled-tasks/{id}/run-now",
            aide::axum::routing::post_with(handlers::run_now, handlers::run_now_docs),
        )
        .api_route(
            "/scheduled-tasks/{id}/runs",
            get_with(handlers::list_task_runs, handlers::list_task_runs_docs),
        )
        .api_route(
            "/scheduler/admin-settings",
            get_with(handlers::get_admin_settings, handlers::get_admin_settings_docs)
                .put_with(handlers::update_admin_settings, handlers::update_admin_settings_docs),
        )
}
