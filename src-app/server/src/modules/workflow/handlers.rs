//! Workflow REST handlers.
//!
//! Phase B2 keeps this empty — only the install handlers in
//! `hub::handlers` are in scope. B6 fills out:
//! - `/api/workflows` (user CRUD + install-from-hub + import + validate)
//! - `/api/workflows/{id}/dry-run` / `/test` / `/run`
//! - `/api/workflows/system` (admin CRUD)
//! - `/api/workflow-runs/{id}` + `/events` + `/output/{step_id}`
//!   + `/artifact/{step_id}/{filename}` + `/cancel` + `/elicit/{id}`

#![allow(dead_code)]
