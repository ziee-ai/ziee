use super::detection::get_gpu_usage_data;
use super::types::{CPUUsage, HardwareUsageUpdate, MemoryUsage};
use axum::response::sse::Event;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::Duration,
};
use sysinfo::System;
use tokio::time::interval;
use uuid::Uuid;

// =====================================================
// SSE Connection Management
// =====================================================

type ClientId = Uuid;

/// Cap the total number of concurrent SSE clients on this endpoint.
/// Without a cap the in-memory client map grows unboundedly — combined
/// with the per-client unbounded mpsc channel, an authenticated user
/// can mint thousands of streams (each backed by their own channel that
/// queues every 2-second broadcast) and OOM the server via channel-
/// backlog growth. Closes 12-hardware F-01 (High).
const MAX_SSE_CLIENTS: usize = 256;

lazy_static::lazy_static! {
    static ref SSE_CLIENTS: Mutex<HashMap<ClientId, tokio::sync::mpsc::UnboundedSender<Result<Event, axum::Error>>>>
        = Mutex::new(HashMap::new());
}

/// Active-monitoring flag. AtomicBool with compare_exchange so the
/// "spawn iff not already running" check is genuinely atomic, closing
/// the TOCTOU window the audit flagged in 12-hardware F-04 (Medium)
/// (the Mutex<bool> variant left a sliver between unlock and spawn
/// where two threads could double-spawn).
static MONITORING_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Result returned by `add_client`: either a fresh receiver, or `None`
/// when the cap has been reached. Callers must convert `None` into an
/// HTTP 429 / 503 response.
pub struct AddClientResult {
    pub receiver: tokio::sync::mpsc::UnboundedReceiver<Result<Event, axum::Error>>,
}

/// Add a new SSE client to the connection pool. Returns None when the
/// global cap (MAX_SSE_CLIENTS) is already at capacity — the caller
/// must surface that as a 429 / 503 to the client.
pub fn add_client(
    client_id: ClientId,
) -> Option<tokio::sync::mpsc::UnboundedReceiver<Result<Event, axum::Error>>> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

    {
        let mut clients = SSE_CLIENTS.lock().unwrap();
        if clients.len() >= MAX_SSE_CLIENTS {
            tracing::warn!(
                client_count = clients.len(),
                "Hardware-monitoring SSE registry full; refusing new client"
            );
            return None;
        }
        clients.insert(client_id, tx);
    }

    tracing::info!(%client_id, "Added hardware monitoring client");
    Some(rx)
}

/// Remove client from connection pool
pub fn remove_client(client_id: ClientId) {
    let mut clients = SSE_CLIENTS.lock().unwrap();
    clients.remove(&client_id);
    tracing::debug!("Removed hardware monitoring client: {}", client_id);
}

/// Start hardware monitoring service
pub async fn start_hardware_monitoring() {
    // Atomic claim — only one task ever wins. Closes 12-hardware F-04.
    if MONITORING_ACTIVE
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return; // Already running
    }

    tracing::info!("Starting hardware monitoring service");

    tokio::spawn(async {
        let mut interval = interval(Duration::from_secs(2)); // Update every 2 seconds
        let mut sys = System::new_all();

        loop {
            interval.tick().await;

            // Check if we have any connected clients
            let client_count = {
                let clients = SSE_CLIENTS.lock().unwrap();
                clients.len()
            };

            if client_count == 0 {
                // No clients connected, stop monitoring.
                tracing::info!("No clients connected, stopping hardware monitoring");
                MONITORING_ACTIVE.store(false, Ordering::SeqCst);
                // Re-check under the relaxed flag: if a client connected
                // during the tiny window between client_count check and
                // the store above, they would have seen the flag still
                // set (and skipped restart). Resurrect ourselves if so.
                let recheck = SSE_CLIENTS.lock().unwrap().len();
                if recheck > 0
                    && MONITORING_ACTIVE
                        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                        .is_ok()
                {
                    tracing::info!(
                        "Hardware monitoring resurrecting — clients reconnected during shutdown window"
                    );
                    continue;
                }
                break;
            }

            // Refresh system information
            sys.refresh_all();

            // Collect usage data
            let usage_update = collect_hardware_usage(&mut sys);

            // Send update to all connected clients
            broadcast_usage_update(usage_update).await;
        }
    });
}

/// Collect current hardware usage
fn collect_hardware_usage(sys: &mut System) -> HardwareUsageUpdate {
    let timestamp = chrono::Utc::now().to_rfc3339();

    // CPU usage (average of all cores)
    let cpu_usage = sys.global_cpu_usage();
    let cpu = CPUUsage {
        usage_percentage: cpu_usage,
        temperature: None, // sysinfo doesn't provide CPU temperature on all platforms
        frequency: sys.cpus().first().map(|cpu| cpu.frequency()),
    };

    // Memory usage
    let total_ram = sys.total_memory();
    let used_ram = sys.used_memory();
    // Saturating subtraction: on Linux, used_memory() can occasionally
    // report a value > total_memory() due to cgroup vs host accounting
    // drift, which would panic in debug + wrap to u64::MAX in release.
    // Closes 12-hardware F-05 (Medium).
    let available_ram = total_ram.saturating_sub(used_ram);
    let usage_percentage = if total_ram > 0 {
        (used_ram as f32 / total_ram as f32) * 100.0
    } else {
        0.0
    };

    let memory = MemoryUsage {
        used_ram,
        available_ram,
        used_swap: Some(sys.used_swap()),
        available_swap: Some(sys.total_swap().saturating_sub(sys.used_swap())),
        usage_percentage,
    };

    // GPU usage (currently returns empty vec)
    let gpu_devices = get_gpu_usage_data();

    HardwareUsageUpdate {
        timestamp,
        cpu,
        memory,
        gpu_devices,
    }
}

/// Broadcast usage update to all connected clients
async fn broadcast_usage_update(usage_update: HardwareUsageUpdate) {
    let clients = {
        let clients = SSE_CLIENTS.lock().unwrap();
        clients.clone()
    };

    if clients.is_empty() {
        return;
    }

    let update_event = super::types::SSEHardwareUsageEvent::Update(usage_update);
    let event: Event = update_event.into();

    // Send to all clients and track disconnected ones
    let mut disconnected_clients = Vec::new();

    for (client_id, tx) in clients.iter() {
        if tx.send(Ok(event.clone())).is_err() {
            disconnected_clients.push(*client_id);
        }
    }

    // Remove disconnected clients
    if !disconnected_clients.is_empty() {
        let mut clients = SSE_CLIENTS.lock().unwrap();
        for client_id in disconnected_clients {
            clients.remove(&client_id);
        }
    }
}
