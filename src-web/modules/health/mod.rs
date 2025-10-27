mod routes;

pub use routes::routes;

use crate::module_api::{AppModule, ModuleContext};
use aide::axum::ApiRouter;
use std::error::Error;

pub struct HealthModule;

impl HealthModule {
    pub fn new() -> Self {
        Self
    }
}

impl AppModule for HealthModule {
    fn name(&self) -> &'static str {
        "health"
    }

    fn init(&mut self, _ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(routes())
    }
}

impl Default for HealthModule {
    fn default() -> Self {
        Self::new()
    }
}
