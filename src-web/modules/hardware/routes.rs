use aide::axum::{routing::get_with, ApiRouter};
use aide::transform::TransformOperation;
use axum::Json;
use sqlx::PgPool;

use crate::modules::permissions::with_permission;

use super::handlers::*;
use super::permissions::{HardwareMonitor, HardwareRead};
use super::types::{HardwareInfoResponse, HardwareUsageUpdate, SSEHardwareUsageEvent};

/// Hardware module routes
pub fn hardware_router() -> ApiRouter<PgPool> {
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

// =====================================================
// OpenAPI Documentation
// =====================================================

/// Documentation for get_hardware_info endpoint
fn get_hardware_info_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HardwareRead,)>(op)
        .id("Hardware.info")
        .summary("Get Hardware Information")
        .description("Get static hardware information including OS, CPU, Memory, and GPU details")
        .tag("Hardware")
        .response::<200, Json<HardwareInfoResponse>>()
        .response::<401, ()>()
        .response::<403, ()>()
}

/// Documentation for subscribe_hardware_usage endpoint
fn subscribe_hardware_usage_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HardwareMonitor,)>(op)
        .id("Hardware.stream")
        .summary("Subscribe to Hardware Usage Stream")
        .description("Subscribe to real-time hardware usage updates via Server-Sent Events (SSE)")
        .tag("Hardware")
        .response::<200, Json<SSEHardwareUsageEvent>>()
        .response::<401, ()>()
        .response::<403, ()>()
}

/// Documentation for types endpoint
fn hardware_types_docs(op: TransformOperation) -> TransformOperation {
    op.description("Types for OpenAPI generation")
        .response::<600, Json<HardwareUsageUpdate>>()
}
