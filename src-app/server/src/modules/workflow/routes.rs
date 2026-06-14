//! Workflow router.
//!
//! Phase B2 mounts an empty router so the module's `register_routes`
//! hook compiles clean. B6 wires the full REST surface (user + admin
//! + workflow-runs) per plan §3.

#![allow(dead_code)]

use aide::axum::ApiRouter;

pub fn workflow_router() -> ApiRouter {
    ApiRouter::new()
}
