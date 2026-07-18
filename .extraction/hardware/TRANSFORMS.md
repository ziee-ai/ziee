# Chunk `hardware` — TRANSFORMS

Every transform applied moving the hardware core into `ziee-hardware`, each with
its decision + resolution. Zero TBD.

## T-1 — `types.rs`: `crate::sse_event_enum!` → `ziee_core::sse_event_enum!`

### Decision — the SSE event enum uses ziee's re-exported macro

`types.rs` defines `SSEHardwareUsageEvent` via `crate::sse_event_enum! { … }`. In
ziee that macro is `pub use ziee_core::sse_event_enum` (lib.rs), so `crate::` is
the app's own re-export. In `ziee-hardware`, `crate::` is the new crate, which has
no such macro.

**Resolution:** changed the single call site to `ziee_core::sse_event_enum!` (the
`#[macro_export]` macro's canonical path; its body uses `$crate::macros::…`, so it
expands identically regardless of the invoking crate). The generated enum body +
methods + `Into<axum::response::sse::Event>` impl are unchanged, and the schemars
derive on `SSEHardwareUsageEvent` keys it by the same short ident → OpenAPI schema
identical. `diff` shows only this one line changed in `types.rs`.

## T-2 — `permissions.rs`: `PermissionCheck` import re-pointed

### Decision — the permission trait lives in `ziee-identity`

`HardwareRead`/`HardwareMonitor` implement `PermissionCheck`, which ziee re-exports
from `ziee-identity` (`crate::modules::permissions::PermissionCheck` is a B1b
shim). The `impl` blocks + the `NAME`/`PERMISSION`/`DESCRIPTION`/`MODULE` consts are
identical.

**Resolution:** changed `use crate::modules::permissions::PermissionCheck;` →
`use ziee_identity::PermissionCheck;`. The permission strings (`hardware::read`,
`hardware::monitor`) are unchanged, so the OpenAPI 403 example (built from the
tuple in the retained `handlers.rs` docs) that feeds the UI `Permissions` enum is
byte-identical. Only the `use` line changed.

## T-3 — `gpu-detect` feature + the 4 GPU deps moved, feature FORWARDED

### Decision — `detection.rs` is feature-gated; ziee's default enables it

`detection.rs` has `#[cfg(feature = "gpu-detect")]` branches that name
`nvml_wrapper`/`opencl3`/`ash`, and ziee's `gpu-detect` feature
(`default = ["gpu-detect", …]`) enables the optional deps `nvml-wrapper`,
`opencl3`, `ash`, `wgpu-hal`. Those deps + the code that uses them moved to
`ziee-hardware`, so the feature must move too — AND ziee must keep enabling it, or
a default ziee build would compile the ELSE branch (`#[cfg(not(feature =
"gpu-detect"))]`), a behavior change. Grep proved the 4 GPU crates are used ONLY
by `detection.rs` (no other server code names them — the `ash::`/`Hash` matches
are false positives).

**Resolution:** `ziee-hardware/Cargo.toml` declares the 4 optional GPU deps +
`[features] gpu-detect = ["nvml-wrapper", "opencl3", "ash", "wgpu-hal"]` (verbatim
from the server's former list). `server/Cargo.toml` DROPS the 4 direct deps and
rewrites its own `gpu-detect = ["ziee-hardware/gpu-detect"]`, so ziee's default
(and the desktop crate, which inherits ziee's default features) compiles the exact
same detection code paths. `detection.rs` is byte-for-byte (0 edits). Verified:
`cargo test -p ziee-hardware --features gpu-detect` → 11 passed / 1 ignored;
`cargo check -p ziee` (gpu-detect on by default) exit 0. The dlopen-runtime GPU
detection stays ON in self-contained builds (memory: don't strip dlopen features).

## T-4 — `collect_hardware_usage` visibility `pub(super)` → `pub`

### Decision — a cross-crate access forced by the module→crate boundary

`collect_hardware_usage` was `pub(super)` in `monitoring.rs` (visible to the
`hardware` module so `handlers.rs` at line 176 calls `super::monitoring::
collect_hardware_usage(&mut sys)`). After the move `super` is no longer the same
scope — the retained app `handlers.rs` reaches it across the crate boundary via the
`super::monitoring` shim, and `pub(super)` in the crate = crate-root only, NOT the
app. The build failed `E0603: function collect_hardware_usage is private`.

**Resolution:** widened to `pub` (the minimum that lets ziee's handler reach it),
with a one-line comment noting the reason. Pure visibility widening — the function
body, signature, and behavior are unchanged; the SSE loop's internal call is
unaffected. Every OTHER item the app accesses (`detect_gpu_devices`, `add_client`,
`remove_client`, `start_hardware_monitoring`, `stop_hardware_monitoring`) was
already `pub` (grep confirmed only `collect_hardware_usage` was `pub(super)`).

## T-5 — `hardware/mod.rs` re-export shim (handlers/routes/registration stay)

### Decision — how `super::{detection,monitoring,permissions,types}` keep resolving

The retained app `handlers.rs` uses `super::detection::detect_gpu_devices`,
`super::monitoring::{add_client, remove_client, start_hardware_monitoring}`,
`super::monitoring::collect_hardware_usage`, `super::permissions::{HardwareMonitor,
HardwareRead}`, `super::types::…`; `routes.rs` uses `super::handlers::*`; `main.rs`
calls `modules::hardware::monitoring::stop_hardware_monitoring()`.

**Resolution:** `mod.rs` replaces the four `pub mod detection/monitoring/
permissions/types;` with `pub use ziee_hardware::{detection, monitoring,
permissions, types};` (a `pub use` of a module makes `hardware::detection` an alias
for `ziee_hardware::detection`, so `super::detection` etc. resolve). `handlers`/
`routes` stay as `pub mod`. All types/fns cross via their already-`pub` surface
(only `collect_hardware_usage` needed widening, T-4). Zero call-site edits outside
`mod.rs` + `Cargo.toml`.

## T-6 — `ziee-hardware` deps

### Decision — what detection + monitoring need

**Resolution:** `ziee-core` (the `sse_event_enum!` macro), `ziee-identity`
(`PermissionCheck`), `serde`/`serde_json`/`schemars` (wire + nvidia-smi JSON
parse), `axum` (the SSE `Event` the macro's `Into` impl + monitoring name),
`sysinfo` (OS/CPU/mem), `tokio` (the monitoring loop), `uuid` (client ids),
`lazy_static` (the client pool), `chrono`, `tracing`; the 4 optional GPU deps
behind `gpu-detect`; `serial_test` dev-dep (the monitoring tests mutate global
statics under `#[serial(hw_monitoring)]`). Versions match the ziee server catalog
so the single `src-app/Cargo.lock` unifies them. No build.rs, no build DB.
