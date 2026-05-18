// OnboardingScreen module — guide completion tracking

pub mod handlers;
mod routes;

pub use routes::onboarding_screen_router;

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use std::error::Error;

use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};

/// Register onboarding_screen module
#[distributed_slice(MODULE_ENTRIES)]
static ONBOARDING_SCREEN_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "onboarding_screen",
    order: 15,
    description: "Guide completion tracking",
    constructor: || Box::new(OnboardingScreenModule::new()),
};

pub struct OnboardingScreenModule;

impl OnboardingScreenModule {
    pub fn new() -> Self {
        Self
    }
}

impl AppModule for OnboardingScreenModule {
    fn name(&self) -> &'static str {
        "onboarding_screen"
    }

    fn init(&mut self, _ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(onboarding_screen_router())
    }
}

impl Default for OnboardingScreenModule {
    fn default() -> Self {
        Self::new()
    }
}
