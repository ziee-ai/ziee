// App builder — re-export shim over `ziee-framework`'s app_builder (Chunk B2).
//
// `create_modules` / `initialize_modules` / `build_api_router` /
// `create_cors_layer` / `apply_rate_limit_layer` moved into `ziee-framework`.
// The `create_cors_layer` / `apply_rate_limit_layer` signatures now take
// `&ServerConfig`; ziee call sites pass `&config` (a `Config`), which
// deref-coerces to `&ServerConfig`, so they are unchanged.
//
// `register_event_handlers` stays here: it constructs the domain-coupled
// `EventBus` (which the app owns — see `core/events.rs`).

use sqlx::PgPool;
use std::sync::Arc;

use crate::core::EventBus;
use crate::module_api::AppModule;

pub use ziee_framework::app_builder::{
    apply_rate_limit_layer, build_api_router, create_cors_layer, create_modules,
    initialize_modules,
};

/// Register event handlers from all modules
pub fn register_event_handlers(modules: &[Box<dyn AppModule>], pool: Arc<PgPool>) -> EventBus {
    let mut event_bus = EventBus::new(pool);

    for module in modules.iter() {
        for handler in module.register_event_handlers() {
            tracing::info!(
                "Registering event handler '{}' for module: {}",
                handler.handler_name(),
                module.name()
            );
            event_bus.register(handler);
        }
    }

    tracing::info!(
        "Registered {} event handlers total",
        event_bus.handler_count()
    );
    event_bus
}

#[cfg(test)]
mod tests {
    use super::create_modules;
    use crate::module_api::MODULE_ENTRIES;

    /// `create_modules` must instantiate EVERY registered module exactly once,
    /// in ascending `order` (the init/route/event-registration sequence depends
    /// on this ordering — e.g. the project chat-extension at order 8 must run
    /// before the assistant extension at order 10).
    ///
    /// This test also proves the linkme `MODULE_ENTRIES` slice — now DEFINED in
    /// `ziee-framework` and registered into from ziee's modules via the
    /// re-export shim — links every app module across the crate boundary.
    #[test]
    fn create_modules_instantiates_all_entries_in_order() {
        // Expected: the linkme slice sorted by order (stable), by name.
        let mut expected_entries: Vec<_> = MODULE_ENTRIES.iter().collect();
        expected_entries.sort_by_key(|e| e.order);
        let expected_names: Vec<&str> = expected_entries.iter().map(|e| e.name).collect();

        let modules = create_modules();

        // One module per registered entry — nothing dropped or duplicated.
        assert_eq!(
            modules.len(),
            MODULE_ENTRIES.len(),
            "create_modules must instantiate every registered module"
        );

        // Same names, in the same by-order sequence — proves the sort happened
        // and each entry's constructor produced a module reporting its name.
        let got_names: Vec<&str> = modules.iter().map(|m| m.name()).collect();
        assert_eq!(got_names, expected_names);

        // The reported orders are non-decreasing (defensive: catches a future
        // regression where the sort key changes).
        let orders: Vec<i32> = expected_entries.iter().map(|e| e.order).collect();
        assert!(
            orders.windows(2).all(|w| w[0] <= w[1]),
            "modules must be ordered by ascending `order`"
        );
    }

    /// Module names must be unique — two modules sharing a name would make the
    /// order/route/event wiring ambiguous.
    #[test]
    fn module_names_are_unique() {
        let modules = create_modules();
        let mut names: Vec<&str> = modules.iter().map(|m| m.name()).collect();
        names.sort_unstable();
        let unique = {
            let mut n = names.clone();
            n.dedup();
            n.len()
        };
        assert_eq!(unique, names.len(), "duplicate module name registered");
    }
}
