# Chunk `hardware` — DRIFT scan (round 1)

Drift = any place moving the hardware detection + monitoring core could diverge
from pre-extraction behavior / surface / output. Each candidate reconciled.

- **DRIFT-1.1** — verdict: none. **detection.rs byte-identity.** Copied then
  `diff <(git show 4a2391732:…/detection.rs) sdk/…/detection.rs` is empty (exit 0).
  The 881-line GPU/CPU/mem probe (trusted-binary allowlist, nvml/opencl/ash paths)
  is unchanged; the 2 detection tests pass under `gpu-detect`.

- **DRIFT-1.2** — verdict: none. **SSE enum macro path.** `types.rs`'s only edit
  is `crate::sse_event_enum!` → `ziee_core::sse_event_enum!`. The macro is
  `#[macro_export]` with a `$crate::…` body, so both paths expand identically
  (`$crate` = ziee_core either way). The `SSEHardwareUsageEvent` variants +
  `event_name`/`data` + `Into<Event>` impl + schemars key are unchanged.

- **DRIFT-1.3** — verdict: none. **gpu-detect feature parity (BEHAVIORAL).** The
  feature + its 4 optional deps moved to `ziee-hardware`; ziee's `gpu-detect`
  forwards via `["ziee-hardware/gpu-detect"]` and ziee's `default` still lists
  `gpu-detect`, so a stock `cargo check -p ziee` compiles the SAME
  `#[cfg(feature="gpu-detect")]` branches (verified exit 0). Desktop inherits
  ziee's default. Grep proved no other server code used those 4 crates. Without
  the forward, ziee would silently compile the `not(gpu-detect)` stub — averted.

- **DRIFT-1.4** — verdict: none. **`collect_hardware_usage` visibility.** Widened
  `pub(super)` → `pub` (required across the crate boundary; E0603 otherwise) — a
  pure visibility change, no body/behavior edit. It was the sole `pub(super)`/
  `pub(crate)` item; every other app-reached fn was already `pub`.

- **DRIFT-1.5** — verdict: none. **Shim transparency.** `mod.rs`'s
  `pub use ziee_hardware::{detection, monitoring, permissions, types};` keeps every
  `super::…` in the retained handlers/routes + `main.rs`'s
  `monitoring::stop_hardware_monitoring()` resolving. ziee + ziee-desktop compile
  exit 0; zero call-site edits outside `mod.rs` + `Cargo.toml`.

- **DRIFT-1.6** — verdict: none. **Security invariants intact.** The
  trusted-binary allowlist (F-06), the `MAX_SSE_CLIENTS=256` cap (F-01), and the
  AtomicBool `compare_exchange` single-spawn (F-04 TOCTOU) moved byte-for-byte; the
  admin-only tripwire warn stays in the retained `handlers.rs`. The monitoring
  tests (cap/free/prune) pass.

- **DRIFT-1.7** — verdict: none. **OpenAPI output (E8, BOTH surfaces).** The moved
  types keep their schemars short-ident keys; the permission strings
  (`hardware::read`/`hardware::monitor`) are unchanged (the 403 example is built in
  the retained handler docs). Regenerated ui + desktop: `types.ts` BYTE-IDENTICAL,
  `openapi.json` canonically-equal (jq -S) vs baseline. Restored via `git checkout`.

- **DRIFT-1.8** — verdict: none. **Build hygiene.** No new warnings in the app
  build. A `gpu-detect`-OFF build (sdk `--workspace` default) shows one
  pre-existing unused-mut at detection.rs:99 — a conditional-compilation artifact
  identical to the app's own `--no-default-features` behavior, not a regression. No
  build DB introduced (0-migration module).

**Unresolved drifts: 0**
