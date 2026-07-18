# Chunk `hardware` — `ziee-hardware` DB-free detection + monitoring (CUT manifest)

Lift the DB-free CORE of ziee's `modules/hardware` — system info detection + a
real-time SSE monitoring broadcaster — into a new `ziee-hardware` SDK crate (N7).
The aide/axum handlers + routes (which bind ziee's concrete `RequirePermissions`
resolver) + the `AppModule` registration stay app-side.

## Design-gate — the hardware surface (core moves, HTTP boundary stays)

`hardware` exposes `GET /api/hardware` (static OS/CPU/mem/GPU info, admin-gated
via `hardware::read`), `GET /api/hardware/usage-stream` (a 2s SSE usage
broadcast, `hardware::monitor`), and `GET /api/hardware/types`. The precision is
in `detection` (trusted-path vendor-binary resolution + nvml/opencl/ash/wgpu-hal
probing) and `monitoring` (a client-capped, atomic-single-spawn SSE fan-out) —
both pure functions of the host, no DB, no app type. The permission KEYS
(`HardwareRead`/`HardwareMonitor`) implement `ziee_identity::PermissionCheck`, and
the wire `types` (incl. the `SSEHardwareUsageEvent` enum via the shared
`ziee_core::sse_event_enum!` macro) are serde/schemars structs. All four move.
The `handlers`/`routes` name `RequirePermissions<…>` / `with_permission::<…>`,
which are ziee type-aliases fixing the concrete `ZieeIdentityResolver` (backed by
the global `Repos`/`JwtService`) — so they STAY in ziee; moving them would be a
rewrite (generic-over-resolver), not an equivalence-preserving move.

## Files move: INTO `ziee-hardware` (submodule `sdk/`, sha 35a6e7f1)

- new: `crates/ziee-hardware/src/types.rs` — OS/CPU/mem/GPU info structs +
  `HardwareUsageUpdate` + `SSEHardwareUsageEvent`. ONE edit: `crate::sse_event_enum!`
  → `ziee_core::sse_event_enum!` (TRANSFORMS T-1); rest byte-for-byte.
- new: `crates/ziee-hardware/src/detection.rs` — GPU/CPU/mem detection (881 lines)
  behind the `gpu-detect` feature. Moved BYTE-FOR-BYTE (2 detection tests).
- new: `crates/ziee-hardware/src/monitoring.rs` — the SSE broadcaster
  (`add_client`/`remove_client`/`start_hardware_monitoring`/
  `stop_hardware_monitoring`/`collect_hardware_usage`) + 10 tests. ONE edit:
  `collect_hardware_usage` `pub(super)` → `pub` (TRANSFORMS T-4).
- new: `crates/ziee-hardware/src/permissions.rs` — `HardwareRead`/`HardwareMonitor`.
  ONE edit: `crate::modules::permissions::PermissionCheck` →
  `ziee_identity::PermissionCheck` (TRANSFORMS T-2).
- new: `crates/ziee-hardware/src/lib.rs` — `pub mod detection/monitoring/
  permissions/types;`.
- new: `crates/ziee-hardware/Cargo.toml` — ziee-core (sse macro) + ziee-identity
  (PermissionCheck) + serde/serde_json/schemars/axum/sysinfo/tokio/uuid/
  lazy_static/chrono/tracing; the 4 optional GPU deps behind a `gpu-detect`
  feature; serial_test dev-dep. Build-DB-free.

## Files changed IN ziee (submodule `src-app/`, staged NOT committed)

- del: `server/src/modules/hardware/{types,detection,monitoring,permissions}.rs`.
- edit: `server/src/modules/hardware/mod.rs` — keeps `pub mod handlers/routes;` +
  the `HardwareModule` registration; adds `pub use ziee_hardware::{detection,
  monitoring, permissions, types};` so every `super::…` path in the retained
  handlers/routes + `main.rs`'s `monitoring::stop_hardware_monitoring()` resolve.
- edit: `server/Cargo.toml` — add the `ziee-hardware` path dep; DROP the 4 direct
  GPU deps (moved); rewrite `gpu-detect = ["ziee-hardware/gpu-detect"]` (forwards
  the feature so a default ziee build compiles the same detection paths); keep
  `sysinfo` (the retained handlers still read it).

## Symbols

- Moved: OS/CPU/mem/GPU info types, `HardwareUsageUpdate`,
  `SSEHardwareUsageConnectedData`, `SSEHardwareUsageEvent`, `detect_gpu_devices`,
  `get_gpu_usage_data`, `add_client`, `remove_client`, `start_hardware_monitoring`,
  `stop_hardware_monitoring`, `collect_hardware_usage`, `HardwareRead`,
  `HardwareMonitor`.
- Schemars keys preserved (short idents) → OpenAPI schemas byte-identical.

## Stays app-side

`hardware/{handlers,routes}.rs` (bind `RequirePermissions`/`with_permission` +
`crate::common::{ApiResult,AppError}`) + `mod.rs` registration (name "hardware",
order 75).
