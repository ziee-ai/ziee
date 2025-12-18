//! Auth Module
//!
//! Desktop authentication and user management

mod handlers;
mod routes;

use crate::module_api::DesktopModule;
use anyhow::Result;
use tauri::App;
use ziee_chat::Router;

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

    fn register_routes(&self, router: Router) -> Router {
        tracing::info!("Registering auth routes");
        router.merge(routes::auth_routes())
    }
}
