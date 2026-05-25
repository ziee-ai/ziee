// Hardware handlers and request/response models

use aide::transform::TransformOperation;
use axum::{
    Json, debug_handler,
    response::sse::{Event, Sse},
};
use futures_util::stream::Stream;
use sysinfo::System;
use uuid::Uuid;

use crate::common::{ApiResult, AppError};
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
/// Get static hardware information.
///
/// SECURITY: returns kernel_version, CPU brand, NVIDIA driver version,
/// CUDA version — a textbook CVE-pivot fingerprint surface. The
/// `hardware::read` permission is NOT in the default Users group
/// (migration 1 + 27 confirm), so today only the Administrators group
/// can reach this endpoint. If you ever add `hardware::read` to a
/// non-admin group, the audit (12-hardware F-02 High) recommends
/// splitting into 'summary' (CPU count, RAM size — non-sensitive) vs
/// 'detailed' (versions) tiers — see the audit doc for the design
/// sketch.
///
/// As a tripwire, this handler emits a tracing::warn when a non-admin
/// hits it; that's the signal a delegation has happened and the split
/// is now needed.
#[debug_handler]
pub async fn get_hardware_info(
    auth: RequirePermissions<(HardwareRead,)>,
) -> ApiResult<Json<HardwareInfoResponse>> {
    if !auth.user.is_admin {
        tracing::warn!(
            user_id = %auth.user.id,
            "Non-admin user accessed /api/hardware (detailed info). \
             Consider splitting the endpoint into summary vs detailed \
             tiers — see 12-hardware F-02."
        );
    }
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
        cores: sysinfo::System::physical_core_count().unwrap_or(cpus.len()),
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
}

/// GET /api/hardware/usage-stream
/// SSE endpoint for real-time hardware usage monitoring
#[debug_handler]
pub async fn subscribe_hardware_usage(
    _auth: RequirePermissions<(HardwareMonitor,)>,
) -> ApiResult<Sse<impl Stream<Item = Result<Event, axum::Error>>>> {
    let client_id = Uuid::new_v4();
    // Capped registry — closes 12-hardware F-01.
    let mut rx = add_client(client_id).ok_or_else(|| {
        AppError::new(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "TOO_MANY_CLIENTS",
            "Hardware-monitoring stream pool is at capacity; try again later",
        )
    })?;

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
        tracing::debug!("Hardware monitoring client disconnected: {}", client_id);
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
