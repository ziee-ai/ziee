mod errors;
mod models;
mod routes;
mod service;

#[allow(unused_imports)]
pub use errors::*;
#[allow(unused_imports)]
pub use models::*;
pub use routes::{routes, AuthState};
pub use service::AuthService;

use crate::module_api::{AppModule, ModuleContext};
use crate::modules::user::{UserRepository, UserService};
use aide::axum::ApiRouter;
use std::error::Error;
use std::sync::Arc;

pub struct AuthModule {
    state: Option<AuthState>,
}

impl AuthModule {
    pub fn new() -> Self {
        Self { state: None }
    }
}

impl AppModule for AuthModule {
    fn name(&self) -> &'static str {
        "auth"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        // Get database pool from context
        let pool = ctx.db_pool.as_ref().clone();

        // Create repositories and services
        let user_repository = UserRepository::new(pool.clone());
        let user_service = UserService::new(user_repository.clone());
        let auth_service = Arc::new(AuthService::new(user_repository, user_service));

        // Store state
        self.state = Some(AuthState {
            service: auth_service,
        });

        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        if let Some(state) = &self.state {
            router.merge(routes(state.clone()))
        } else {
            router
        }
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["user"]
    }
}

impl Default for AuthModule {
    fn default() -> Self {
        Self::new()
    }
}
