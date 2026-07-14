# Chunk `hardware` — TESTS-MOVED

## Moved INTO `ziee-hardware` (with the code)

### `crates/ziee-hardware/src/monitoring.rs` `#[cfg(test)]` (10, verbatim)

| Test | Covers |
|---|---|
| `add_client_enforces_cap_and_remove_frees_a_slot` | MAX_SSE_CLIENTS cap + free-on-remove |
| `add_client_enforces_cap_and_remove_frees_slot` | cap + slot reuse |
| `add_client_enforces_global_cap_and_frees_on_remove` | global cap enforcement |
| `add_then_remove_client_updates_the_sse_pool` | pool add/remove bookkeeping |
| `broadcast_usage_update` | broadcast to live client |
| `broadcast_usage_update_delivers_to_live_client_and_prunes_dead` | delivery + dead-conn prune |
| `collect_hardware_usage_produces_wellformed_snapshot` | per-tick snapshot shape |
| `collect_hardware_usage_returns_a_valid_snapshot` | snapshot validity/ranges |
| `start_hardware_monitoring` | spawn path |
| `start_is_idempotent_and_idle_stops_without_clients` | AtomicBool single-spawn + idle stop |

(Run under `#[serial(hw_monitoring)]` — they mutate the process-global SSE pool +
MONITORING_ACTIVE flag; the `serial_test` dev-dep moved with them.)

### `crates/ziee-hardware/src/detection.rs` `#[cfg(test)]` (2, verbatim, gpu-detect-gated)

| Test | Covers |
|---|---|
| `detect_gpu_devices_returns_well_formed_rows` | GPU row detection shape |
| `get_gpu_usage_data_percentages_are_in_range` | usage percentages 0–100 |

SDK result: `cargo test -p ziee-hardware --features gpu-detect` → **11 passed, 1
ignored** (the 1 ignored is the real-hardware-gated case that predates this chunk).

## Stayed in ziee (app-coupled — not moved)

The HTTP-handler tests live in ziee's integration suite (`tests/hardware/…`, if
any) and drive the retained `handlers.rs`/`routes.rs` through the app's
`TestServer` (which names `RequirePermissions`/`module_api`). Unaffected — the
routes, schemas, and permission gates are byte-identical.

No behavioral assertion was edited (only test relocation + the `serial_test`
dev-dep) — the MOVE-preserves-behavior discipline holds.
