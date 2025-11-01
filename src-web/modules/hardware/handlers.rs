// Hardware handlers and request/response models

use aide::transform::TransformOperation;
use axum::{
    debug_handler,
    extract::State,
    response::sse::{Event, Sse},
    Json,
};
use futures_util::stream::Stream;
use sqlx::PgPool;
use sysinfo::System;
use uuid::Uuid;

use crate::common::ApiResult;
use crate::modules::permissions::{RequirePermissions, with_permission};

use super::detection::detect_gpu_devices;
use super::monitoring::{add_client, remove_client, start_hardware_monitoring};
use super::permissions::{HardwareMonitor, HardwareRead};
use super::types::{
    CPUInfo, HardwareInfo, HardwareInfoResponse, HardwareUsageUpdate, MemoryInfo,
    OperatingSystemInfo, SSEHardwareUsageConnectedData, SSEHardwareUsageEvent,
};

// =====================================================
// Route Handlers
// =====================================================

/// GET /api/hardware
/// Get static hardware information
#[debug_handler]
pub async fn get_hardware_info(
    _auth: RequirePermissions<(HardwareRead,)>,
    State(_pool): State<PgPool>,
) -> ApiResult<Json<HardwareInfoResponse>> {
    let mut sys = System::new_all();
    sys.refresh_all();

    // Get OS information
    let operating_system = OperatingSystemInfo {
        name: System::name().unwrap_or_else(|| "Unknown".to_string()),
        version: System::os_version().unwrap_or_else(|| "Unknown".to_string()),
        kernel_version: System::kernel_version(),
        architecture: std::env::consts::ARCH.to_string(),
    };

    // Get CPU information
    let cpus = sys.cpus();
    let cpu = CPUInfo {
        model: cpus
            .first()
            .map(|cpu| cpu.brand().to_string())
            .unwrap_or_else(|| "Unknown".to_string()),
        architecture: std::env::consts::ARCH.to_string(),
        cores: sys.physical_core_count().unwrap_or(cpus.len()),
        threads: Some(cpus.len()),
        base_frequency: cpus.first().map(|cpu| cpu.frequency()),
        max_frequency: None, // sysinfo doesn't provide max frequency directly
    };

    // Get Memory information
    let memory = MemoryInfo {
        total_ram: sys.total_memory(),
        total_swap: Some(sys.total_swap()),
    };

    // Get GPU information
    let gpu_devices = detect_gpu_devices();

    let hardware_info = HardwareInfo {
        operating_system,
        cpu,
        memory,
        gpu_devices,
    };

    Ok((
        axum::http::StatusCode::OK,
        Json(HardwareInfoResponse {
            hardware: hardware_info,
        }),
    ))
}

/// Documentation for get_hardware_info endpoint
pub fn get_hardware_info_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HardwareRead,)>(op)
        .id("Hardware.info")
        .summary("Get Hardware Information")
        .description("Get static hardware information including OS, CPU, Memory, and GPU details")
        .tag("Hardware")
        .response::<200, Json<HardwareInfoResponse>>()
        .response::<401, ()>()
        .response::<403, ()>()
}

/// GET /api/hardware/usage-stream
/// SSE endpoint for real-time hardware usage monitoring
#[debug_handler]
pub async fn subscribe_hardware_usage(
    _auth: RequirePermissions<(HardwareMonitor,)>,
    State(_pool): State<PgPool>,
) -> ApiResult<Sse<impl Stream<Item = Result<Event, axum::Error>>>> {
    let client_id = Uuid::new_v4();
    let mut rx = add_client(client_id);

    // Start monitoring if not already active
    start_hardware_monitoring().await;

    // Create the SSE stream with proper cleanup
    let stream = async_stream::stream! {
        // Send initial connected event
        let connected_event = SSEHardwareUsageEvent::Connected(SSEHardwareUsageConnectedData {
            message: "Hardware monitoring connected".to_string(),
        });
        let event: Event = connected_event.into();
        yield Ok(event);

        // Stream updates from monitoring service
        while let Some(event) = rx.recv().await {
            yield event;
        }

        // Stream ended, remove client
        println!("Hardware monitoring client disconnected: {}", client_id);
        remove_client(client_id);
    };

    Ok((axum::http::StatusCode::OK, Sse::new(stream)))
}

/// Documentation for subscribe_hardware_usage endpoint
pub fn subscribe_hardware_usage_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HardwareMonitor,)>(op)
        .id("Hardware.stream")
        .summary("Subscribe to Hardware Usage Stream")
        .description("Subscribe to real-time hardware usage updates via Server-Sent Events (SSE)")
        .tag("Hardware")
        .response::<200, Json<SSEHardwareUsageEvent>>()
        .response::<401, ()>()
        .response::<403, ()>()
}

/// Dummy endpoint for type generation - ensures SSE types are included in OpenAPI spec
pub async fn hardware_types() -> Json<HardwareUsageUpdate> {
    unreachable!("This endpoint is only for OpenAPI type generation")
}

/// Documentation for types endpoint
pub fn hardware_types_docs(op: TransformOperation) -> TransformOperation {
    op.description("Types for OpenAPI generation")
        .response::<600, Json<HardwareUsageUpdate>>()
}
