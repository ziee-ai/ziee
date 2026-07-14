// Onboarding module — guide completion tracking.
//
// Chunk sdk-surfaces moved the generic, domain-free half — the `OnboardingProgress`
// wire type, the `OnboardingRepository` (compile-time `query!` over
// `user_onboarding`), and the `user_onboarding` table migration — into the
// `ziee-onboarding` SDK crate (which carries its own build DB). ziee stays a thin
// consumer: it re-exports those two symbols (so `Repos.onboarding`, the handlers,
// and the `OnboardingProgress` OpenAPI schema are byte-unchanged) and keeps the
// sync-coupled handlers/routes/registration + the `user_id → users(id)` FK
// migration (`migrations/*_onboarding_fkeys.sql`) app-side.

pub mod handlers;
mod routes;

pub use routes::onboarding_router;
pub use ziee_onboarding::{OnboardingRepository, models};

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use std::error::Error;

use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};

/// Register onboarding module
#[distributed_slice(MODULE_ENTRIES)]
static ONBOARDING_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "onboarding",
    order: 15,
    description: "Guide completion tracking",
    constructor: || Box::new(OnboardingModule::new()),
};

pub struct OnboardingModule;

impl OnboardingModule {
    pub fn new() -> Self {
        Self
    }
}

impl AppModule for OnboardingModule {
    fn name(&self) -> &'static str {
        "onboarding"
    }

    fn init(&mut self, _ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(onboarding_router())
    }
}

impl Default for OnboardingModule {
    fn default() -> Self {
        Self::new()
    }
}
