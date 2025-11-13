use aide::axum::{ApiRouter, routing::get_with};

use super::handlers::*;

/// Hardware module routes
pub fn hardware_router() -> ApiRouter {
    ApiRouter::new()
        .api_route(
            "/hardware",
            get_with(get_hardware_info, get_hardware_info_docs),
        )
        .api_route(
            "/hardware/usage-stream",
            get_with(subscribe_hardware_usage, subscribe_hardware_usage_docs),
        )
        .api_route(
            "/hardware/types",
            get_with(hardware_types, hardware_types_docs),
        )
}
