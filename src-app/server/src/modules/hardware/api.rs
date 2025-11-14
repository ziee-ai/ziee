// Hardware API infrastructure
#![allow(dead_code)]

use super::detection::detect_gpu_devices;
use super::monitoring::{add_client, remove_client, start_hardware_monitoring};
use super::permissions::{HardwareMonitor, HardwareRead};
use super::types::{
    CPUInfo, HardwareInfo, HardwareInfoResponse, MemoryInfo, OperatingSystemInfo,
    SSEHardwareUsageConnectedData, SSEHardwareUsageEvent,
};
use crate::common::ApiResult;
use crate::modules::permissions::RequirePermissions;
use axum::{
    Json, debug_handler,
    response::sse::{Event, Sse},
};
use futures_util::stream::Stream;
use sysinfo::System;
use uuid::Uuid;

// =====================================================
// API Handlers
// =====================================================

/// Get static hardware information
#[debug_handler]
pub async fn get_hardware_info(
    _auth: RequirePermissions<(HardwareRead,)>,
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

/// SSE endpoint for real-time hardware usage monitoring
#[debug_handler]
pub async fn subscribe_hardware_usage(
    _auth: RequirePermissions<(HardwareMonitor,)>,
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
