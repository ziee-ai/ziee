//! Auth Module
//!
//! Desktop authentication and user management

pub mod commands;
mod handlers;
mod routes;

use crate::module_api::DesktopModule;
use anyhow::Result;
use tauri::App;
use ziee_chat::ApiRouter;

pub struct AuthModule;

impl AuthModule {
    pub fn new() -> Self {
        Self
    }
}

impl DesktopModule for AuthModule {
    fn name(&self) -> &'static str {
        "auth"
    }

    fn description(&self) -> &'static str {
        "Desktop authentication and user management"
    }

    fn init(&mut self, _app: &mut App) -> Result<()> {
        tracing::info!("Auth module initialized");
        Ok(())
    }

    fn register_api_routes(&self, router: ApiRouter) -> ApiRouter {
        tracing::info!("Registering auth API routes");
        router.merge(routes::auth_api_routes())
    }
}
