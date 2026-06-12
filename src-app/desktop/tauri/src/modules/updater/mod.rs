//! Auto-Updater Module
//!
//! Application auto-update functionality via HTTP routes

mod handlers;
mod routes;

use crate::module_api::DesktopModule;
use anyhow::Result;
use tauri::App;
use ziee::ApiRouter;

pub struct UpdaterModule;

impl UpdaterModule {
    pub fn new() -> Self {
        Self
    }
}

impl DesktopModule for UpdaterModule {
    fn name(&self) -> &'static str {
        "updater"
    }

    fn description(&self) -> &'static str {
        "Application auto-update via HTTP routes"
    }

    fn init(&mut self, _app: &mut App) -> Result<()> {
        tracing::info!("Updater module initialized");
        Ok(())
    }

    fn register_api_routes(&self, router: ApiRouter) -> ApiRouter {
        tracing::info!("Registering updater API routes");
        router.merge(routes::updater_api_routes())
    }
}
