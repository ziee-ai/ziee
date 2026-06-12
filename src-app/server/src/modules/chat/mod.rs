// Chat module - Modular architecture for AI chat functionality

use aide::axum::ApiRouter;
use axum::Extension;
use linkme::distributed_slice;
use sqlx::PgPool;
use std::error::Error;
use std::sync::Arc;

use crate::ModuleContext;
use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleEntry};

pub mod core;
pub mod extensions;
pub mod stream;

// Include auto-generated extension registration code
#[path = "extension_registration.rs"]
mod extension_registration;
use extension_registration::auto_register_extensions;

// Re-exports
pub use core::extension::ExtensionRegistry;
pub use core::routes::chat_router;

/// Register chat module
#[distributed_slice(MODULE_ENTRIES)]
static CHAT_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "chat",
    order: 50,
    description: "Core chat functionality and message handling",
    constructor: || Box::new(ChatModule::new()),
};

/// Chat Module
/// Manages chat conversations, messages, and extensions
pub struct ChatModule {
    pool: Option<Arc<PgPool>>,
    extension_registry: Option<Arc<ExtensionRegistry>>,
}

impl ChatModule {
    pub fn new() -> Self {
        Self {
            pool: None,
            extension_registry: None,
        }
    }
}

impl AppModule for ChatModule {
    fn name(&self) -> &'static str {
        "chat"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        self.pool = Some(ctx.db_pool.clone());

        // Auto-register extensions using generated code
        // Extensions are discovered at build time and registered in order based on METADATA.order
        let registry = auto_register_extensions((*ctx.db_pool).clone(), ctx.config.clone());
        self.extension_registry = Some(Arc::new(registry));

        tracing::info!("Chat module initialized with auto-registered extensions");
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        if let Some(_pool) = &self.pool {
            if let Some(registry) = &self.extension_registry {
                // First, register extension routes (extensions may add their own endpoints)
                let router_with_extension_routes = registry.register_routes(router);

                // Then create chat router with pool state and extension
                // registry as extension. `mcp_defaults_router` was
                // explicitly merged here in the past; it's now
                // contributed by the mcp bridge's ChatExtension::register_routes
                // (the line above) so chat doesn't have to know it exists.
                let chat_module_router = ApiRouter::new()
                    .merge(chat_router())
                    .layer(Extension(registry.clone()));

                // The chat-token stream + its subscription control. No
                // extension layer needed; merge on the outer router.
                router_with_extension_routes
                    .merge(chat_module_router)
                    .merge(stream::chat_stream_router())
            } else {
                tracing::error!(
                    "ChatModule: Extension registry not initialized during route registration"
                );
                router
            }
        } else {
            tracing::error!("ChatModule: Pool not initialized during route registration");
            router
        }
    }
}

impl Default for ChatModule {
    fn default() -> Self {
        Self::new()
    }
}
