// Onboarding module — guide completion tracking

pub mod handlers;
pub mod models;
mod repository;
mod routes;

pub use repository::OnboardingRepository;
pub use routes::onboarding_router;

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
