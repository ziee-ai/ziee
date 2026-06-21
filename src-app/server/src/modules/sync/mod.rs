//! Realtime cross-device sync module.
//!
//! Exposes a per-user SSE stream (`GET /api/sync/subscribe`) that pushes
//! lightweight `{entity, action, id}` change notifications. Mutating
//! handlers call [`publish`] with an explicit `Audience` chosen at the call
//! site (there is NO central audience table — see `event.rs`); the per-user
//! keyed `registry` routes delivery + suppresses self-echo. Notify-and-refetch
//! only — no row data crosses the wire; clients refetch via the existing
//! permission-checked REST endpoints.

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use std::error::Error;

use crate::ModuleContext;
use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleEntry};

pub mod event;
pub mod extractor;
pub mod handlers;
pub mod registry;

pub use event::{Audience, PermRule, SyncAction, SyncEntity, publish, publish_session_to_users};
pub use extractor::SyncOrigin;

/// Register the sync module via linkme.
#[distributed_slice(MODULE_ENTRIES)]
static SYNC_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "sync",
    // Leaf SSE module with no init-ordering dependencies; the value is
    // otherwise arbitrary.
    order: 95,
    description: "Realtime cross-device sync over SSE",
    constructor: || Box::new(SyncModule::new()),
};

pub struct SyncModule;

impl SyncModule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SyncModule {
    fn default() -> Self {
        Self::new()
    }
}

impl AppModule for SyncModule {
    fn name(&self) -> &'static str {
        "sync"
    }

    fn description(&self) -> &'static str {
        "Realtime cross-device sync over SSE"
    }

    fn init(&mut self, _ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(handlers::sync_router())
    }
}
