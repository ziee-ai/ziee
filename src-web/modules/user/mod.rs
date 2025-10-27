mod errors;
mod models;
mod repository;
mod routes;
mod service;

#[allow(unused_imports)]
pub use errors::*;
#[allow(unused_imports)]
pub use models::*;
pub use repository::UserRepository;
pub use routes::{routes, UserState};
pub use service::UserService;

use crate::module_api::{AppModule, ModuleContext};
use aide::axum::ApiRouter;
use std::error::Error;
use std::sync::Arc;

pub struct UserModule {
    state: Option<UserState>,
}

impl UserModule {
    pub fn new() -> Self {
        Self { state: None }
    }
}

impl AppModule for UserModule {
    fn name(&self) -> &'static str {
        "user"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        // Get database pool from context
        let pool = ctx.db_pool.as_ref().clone();

        // Create repository and service
        let repository = UserRepository::new(pool);
        let service = Arc::new(UserService::new(repository));

        // Store state
        self.state = Some(UserState { service });

        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        if let Some(state) = &self.state {
            router.merge(routes(state.clone()))
        } else {
            router
        }
    }
}

impl Default for UserModule {
    fn default() -> Self {
        Self::new()
    }
}
