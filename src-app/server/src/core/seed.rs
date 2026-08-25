//! ziee's app-side wiring for the `ziee-seed` SDK engine.
//!
//! The engine (crate `ziee_seed`) owns the domain-neutral machinery — the
//! `SeedProvider` seam + `SEED_PROVIDERS` distributed-slice, the layered YAML
//! loader + deep-merge, `${ENV_VAR}` templating, the `seed_ledger` ownership
//! ledger, and the reconcile/dump directives. This module supplies the *app*
//! half:
//!
//! 1. [`DEFAULT_SEED_YAML`] — ziee's embedded Layer-0 default (decision N9: the
//!    engine embeds no data; the app passes its own).
//! 2. [`seed_config`] — the deploy-level [`ziee_seed::SeedConfig`] (default
//!    enabled; global reconcile stays driven by the `SEED_RECONCILE` env var,
//!    which `ziee_seed::run` reads itself).
//! 3. [`run`] — the boot entrypoint both `setup_server` (desktop/embedded) and
//!    `main` (server binary) call in the post-migration window (after
//!    `init_storage_key`, before serving).
//! 4. The singleton **settings** providers — the uniform special case (one
//!    CHECK-enforced row, UPDATE-declared-columns only) declared centrally as
//!    [`ziee_seed::GenericSingleton`]s rather than scattered `seed.rs` files.
//!    The multi-row families (llm_providers / mcp_servers / assistants) are
//!    module-owned (`modules/*/seed.rs`).

use std::sync::Arc;

use linkme::distributed_slice;
use sqlx::PgPool;
use ziee_seed::{GenericSingleton, Kind, SeedConfig, SeedEntry, SeedProvider, SingletonSeedProvider, SEED_PROVIDERS};

/// ziee's embedded Layer-0 default seed, baked into the binary.
pub const DEFAULT_SEED_YAML: &str = include_str!("../../resources/seed/default.yaml");

/// The deploy-level seed configuration. `reconcile` stays `false` here (the
/// safe seed-if-empty default); `ziee_seed::run` still promotes to reconcile
/// when the `SEED_RECONCILE` env var is truthy, and an operator overlay file is
/// picked up from the `SEED_FILE` env var — so ziee needs no bespoke config
/// section to get the standard operator ergonomics.
pub fn seed_config() -> SeedConfig {
    SeedConfig::default()
}

/// Boot the declarative seed against a migrated pool. Called after
/// `init_storage_key`, before the server serves. A bad *requested overlay* is a
/// fatal boot error (the caller propagates it); a single failing provider is
/// logged + skipped inside the engine, never fatal.
pub async fn run(pool: &PgPool) -> Result<(), String> {
    ziee_seed::run(pool, &seed_config(), DEFAULT_SEED_YAML).await
}

// ─────────────────────────── singleton settings registry ───────────────────────────
//
// Each is an independent `SEED_PROVIDERS` entry. Orders sit ABOVE the multi-row
// families (groups referenced first at 10, providers at 30, mcp at 35,
// assistants at 45) so the settings run last — they reference nothing.

fn singleton(
    section: &'static str,
    table: &'static str,
    pk_where: &'static str,
    columns: &'static [(&'static str, Kind)],
) -> Arc<dyn SeedProvider> {
    Arc::new(SingletonSeedProvider(GenericSingleton { section, table, pk_where, columns }))
}

// web_search_settings — BOOLEAN pk (id = TRUE). Full migration-verified whitelist
// (schema: 202607140225_web_search_schema.sql). This is the singleton the seed
// integration test reconciles.
const WEB_SEARCH_COLS: &[(&str, Kind)] = &[
    ("enabled", Kind::Bool),
    ("provider_chain", Kind::TextArray),
    ("max_results", Kind::Int),
    ("fetch_max_bytes", Kind::BigInt),
    ("fetch_max_chars", Kind::Int),
    ("request_timeout_secs", Kind::Int),
];
#[distributed_slice(SEED_PROVIDERS)]
static S_WEB_SEARCH: SeedEntry = SeedEntry {
    section: "web_search_settings",
    order: 52,
    factory: || singleton("web_search_settings", "web_search_settings", "id = TRUE", WEB_SEARCH_COLS),
};

// code_sandbox_settings — BOOLEAN pk (id = TRUE). Operator-tunable resource caps.
const CODE_SANDBOX_COLS: &[(&str, Kind)] = &[
    ("memory_max_bytes", Kind::BigInt),
    ("memory_swap_max_bytes", Kind::BigInt),
    ("pids_max", Kind::Int),
    ("cpu_max", Kind::Text),
    ("timeout_secs", Kind::Int),
    ("vm_idle_evict_secs", Kind::Int),
];
#[distributed_slice(SEED_PROVIDERS)]
static S_CODE_SANDBOX: SeedEntry = SeedEntry {
    section: "code_sandbox_settings",
    order: 50,
    factory: || singleton("code_sandbox_settings", "code_sandbox_settings", "id = TRUE", CODE_SANDBOX_COLS),
};

// session_settings — BOOLEAN pk (id = TRUE), owned by ziee-auth's schema. Token
// lifetimes are the only operator-tunable columns.
const SESSION_COLS: &[(&str, Kind)] = &[
    ("access_token_expiry_hours", Kind::Int),
    ("refresh_token_expiry_days", Kind::Int),
];
#[distributed_slice(SEED_PROVIDERS)]
static S_SESSION: SeedEntry = SeedEntry {
    section: "session_settings",
    order: 51,
    factory: || singleton("session_settings", "session_settings", "id = TRUE", SESSION_COLS),
};
