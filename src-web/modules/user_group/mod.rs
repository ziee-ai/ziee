mod errors;
mod models;
mod repository;
mod routes;
mod service;

#[allow(unused_imports)]
pub use errors::*;
#[allow(unused_imports)]
pub use models::*;
pub use repository::UserGroupRepository;
pub use routes::{routes, UserGroupState};
pub use service::UserGroupService;

use crate::module_api::{AppModule, ModuleContext};
use aide::axum::ApiRouter;
use std::error::Error;
use std::sync::Arc;

pub struct UserGroupModule {
    state: Option<UserGroupState>,
}

impl UserGroupModule {
    pub fn new() -> Self {
        Self { state: None }
    }
}

impl AppModule for UserGroupModule {
    fn name(&self) -> &'static str {
        "user_group"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        // Get database pool from context
        let pool = ctx.db_pool.as_ref().clone();

        // Create repository and service
        let repository = UserGroupRepository::new(pool);
        let service = Arc::new(UserGroupService::new(repository));

        // Store state
        self.state = Some(UserGroupState { service });

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

impl Default for UserGroupModule {
    fn default() -> Self {
        Self::new()
    }
}
