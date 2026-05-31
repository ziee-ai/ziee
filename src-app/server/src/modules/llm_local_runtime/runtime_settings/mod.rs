//! Singleton runtime-settings row.
//!
//! Stores the three operator-tunable knobs:
//!  - `idle_unload_secs` — reaper threshold
//!  - `auto_start_timeout_secs` — how long ensure_running waits for Healthy
//!  - `drain_timeout_secs` — how long the reaper waits for in-flight to drain
//!
//! Persistence lives on `LocalRuntimeRepository`
//! (`get_runtime_settings` / `update_runtime_settings`), reached via
//! `Repos.local_runtime` — there is no separate settings repository.

pub mod handlers;
pub mod models;
