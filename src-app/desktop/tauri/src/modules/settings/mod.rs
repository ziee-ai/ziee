//! Settings Module
//!
//! Desktop-specific settings management

mod handlers;
mod routes;

use crate::module_api::DesktopModule;
use anyhow::Result;
use tauri::App;
use ziee_chat::ApiRouter;

pub struct SettingsModule;

impl SettingsModule {
    pub fn new() -> Self {
        Self
    }
}

impl DesktopModule for SettingsModule {
    fn name(&self) -> &'static str {
        "settings"
    }

    fn description(&self) -> &'static str {
        "Desktop-specific settings management"
    }

    fn init(&mut self, _app: &mut App) -> Result<()> {
        tracing::info!("Settings module initialized");
        Ok(())
    }

    fn register_api_routes(&self, router: ApiRouter) -> ApiRouter {
        tracing::info!("Registering settings API routes");
        router.merge(routes::settings_api_routes())
    }
}
