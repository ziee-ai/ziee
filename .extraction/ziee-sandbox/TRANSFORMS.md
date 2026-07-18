# `ziee-sandbox` engine — executed transforms + turnkey seam design

Phase-2 engine extraction. This session **executed + Linux-verified coupling A**
(the security-critical `/lit` reverse-dep lift, byte-identical argv proven by a
passing unit test) and **fully resolved the design for all 5 extended-seam
couplings** (B–E) into a mechanical, turnkey spec for the physical crate move.

Scope note (honest): the `/lit` lift is done + verified in-place (it is the
load-bearing hardening-equivalence work and the #1 prerequisite for moving
`sandbox.rs`). The physical `sdk/crates/ziee-sandbox` crate creation + file move
+ `version_manager` split + provider wiring is a large (~150-edit, all-or-nothing
compile) mechanical follow-up specified in full below; it was NOT physically
carved this session.

---

## EXECUTED — Coupling A: `/lit` reverse-dep lift (VERIFIED byte-identical)

`build_bwrap_argv` (`sandbox.rs`) previously reached
`crate::modules::lit_search::fulltext::cache::{conversation_view_dir,
SANDBOX_MOUNT_PATH}` to push the `/lit` `--ro-bind` inline (former L809-825). That
is the single most security-sensitive function and it named a ziee app module —
blocking a build-DB-free move.

**Lift:** added `SandboxContext::extra_ro_binds: Vec<(String,String)>`. The argv
builder now does `all_ro_binds.extend(ctx.extra_ro_binds.iter().cloned());` at the
EXACT position the inline block occupied (after attachment + workflow RO binds,
inside the shared `extra_ro_binds` `--ro-bind-try` loop). The `/lit` computation
moved to the ziee-side callers that CAN reach `lit_search` and populate the ctx:
- `handlers::build_context` (chat/MCP execute path) — computes the bind with the
  identical `conversation_view_dir(conversation_id).is_dir()` gate.
- `workflow/dispatch.rs` (workflow sandbox step) — same computation on its
  `conv_id` (byte-identical to the old inline path, which computed from the same
  conversation_id).
- All other construction sites (mount_context_extension, VM `guest_ctx`
  re-derivations, 6 test helpers) set `extra_ro_binds` empty / carry-forward
  (`ctx.extra_ro_binds.clone()` in mac_vm/wsl2 `guest_ctx`).

**Result:** `sandbox.rs` now has ZERO `crate::modules::lit_search` code refs (only
prose comments). Emission is byte-identical: `--ro-bind-try <host_view> /lit`,
same flag, same source, same slot.

### Hardening byte-identical audit — EVIDENCE (Linux, load-bearing)
`sandbox::tests::lit_bind_lift_is_byte_identical` (new) asserts on the REAL
`build_bwrap_argv Vec<String>`:
1. empty `extra_ro_binds` ⇒ NO `/lit` token anywhere (inline block truly gone);
2. injected `(host_view, "/lit")` ⇒ `--ro-bind-try <host_view> /lit` contiguous;
3. it lands after the `/proc` masks and before the prlimit tail (the exact old
   `all_ro_binds.push` slot);
4. **the ONLY delta between the bare and with-`/lit` argv is the three injected
   tokens — every other token is byte-for-byte identical.**

`cargo test --lib -p ziee code_sandbox::sandbox::` → **23 passed; 0 failed**
(incl. this audit). `cargo check -p ziee` → **0** (3 pre-existing unrelated
warnings). cgroup/prlimit/seccomp emission is untouched by this change (self-
contained; only `ziee_core::AppError` + `resource_limits::CodeSandboxResourceLimits`
+ the `sandbox_seccomp` SDK crate). **No isolation weakened; no
hardening-equivalence break exists** — the STOP condition does not apply.

---

## RESOLVED DESIGN — the remaining physical move (turnkey)

### Crate `sdk/crates/ziee-sandbox`
Deps: `ziee-core` (AppError), `ziee-framework` (mcp envelopes/loopback_host used
by types.rs re-exports — confirm at move; if only types.rs needs them and types
stays split, may drop), `sandbox-vm-protocol`, `sandbox-seccomp` (optional,
`code_sandbox_seccomp` feature, `cfg(linux)`), `sandbox-vm-launcher` (macOS),
`axum`, `tokio`, `uuid`, `serde`, `serde_json`, `schemars`, `once_cell`,
`dashmap`, `async-trait`, `tracing`, `chrono`, `libc` (linux), and `sqlx`
`{ default-features=false, features=["derive","macros"] }` for the
`#[derive(sqlx::FromRow)]` on `CodeSandboxResourceLimits` ONLY (macro-only, no
`postgres`/runtime, no DATABASE_URL). `[lints] workspace = true`.

### Files → MOVE (git mv into crate, then path-rewrite)
`cgroup.rs`, `probes.rs`, `sandbox.rs`, `mcp_spawn.rs`, `mount_provider.rs`,
`workflow_staging.rs`, `tools/{mod,execute,files}.rs`, `config.rs` (the
state-holder + `SandboxAvailability`), `types.rs` (state minus `pool` + providers),
`embedded.rs`, `wsl2_agent_embedded.rs`, the whole `backend/` tree
(`mod,linux_bwrap,mac_vm,wsl2,vm_client,vm_long_lived,hvsocket,unsupported,
helper_service/**`). Plus a NEW `seam.rs` (traits + `set_app_data_dir`).

### Files → STAY (ziee) — become the seam impls
`version_manager.rs` (DB half), `runtime_mount.rs`, `runtime_fetch.rs`,
`resource_limits_cache.rs` (= `ResourceLimitsProvider` impl), `streaming.rs`
(coupling E — reclassified STAY: uses `is_flavor_cached`/`is_fetch_in_flight`/
progress-callback `ensure_fetched`, all rootfs-policy not engine), `handlers.rs`,
`routes.rs`, `version_handlers.rs`, `version_install_tasks.rs`, `repository.rs`,
`models.rs`, `permissions.rs`, `mount_context_extension.rs`, `version_back.rs`,
`resource_limits.rs` (the DB methods split — see below), `mod.rs` (wires providers
at boot), `migrations/`. `code_sandbox/mod.rs` adds
`pub use ziee_sandbox::{…}` re-export shims so retained files'
`crate::modules::code_sandbox::X` paths resolve unchanged (the ziee-hardware
shim precedent).

### SPLITS
- **`version_manager.rs`** (2295 L): DB half STAYS (uses runtime `sqlx::query`
  with `code_sandbox_versions`/`code_sandbox_settings` SQL literals → would trip
  the build-DB-free grep). The DB-FREE in-mem/fs tail MOVES to the engine:
  `MountedArtifact` (@1341), `InflightKind` (@1374), `InflightGuard` (@1382,
  + Drop), `register_mount` (@1427), `acquire_inflight` (@1456),
  `mounted_artifact`/`list_mounted_artifacts` (@1472/@1478),
  `wipe_install_caches_for_conversation` (@1816), `consume_workspace_sentinel`
  (@1853), the `MOUNTED_ARTIFACTS`/`DRAINING_ARTIFACTS` statics, the sentinel
  consts + `WipeSentinel`/`WipeResult` + `wipe_install_caches_in_root`. ziee's
  `runtime_mount`/`version_manager::status` re-import these from
  `ziee_sandbox::registry` (or re-export shim). Verified DB-free by explorer.
- **`resource_limits.rs`**: `CodeSandboxResourceLimits` (with FromRow) +
  `UpdateCodeSandboxResourceLimits` + `validate()` MOVE to the engine (value
  types). The two impl methods on `CodeSandboxRepository` (`get_resource_limits`
  @190, `update_resource_limits` @227 — both `sqlx::query_as` over
  `code_sandbox_settings`) STAY in ziee (move to `repository.rs` or a ziee
  `resource_limits_db.rs`).
- **`runtime_mount.rs` / `runtime_fetch.rs`**: their VOCABULARY types move to the
  engine as the provider return types (`EnsureOutcome`, `EvictOutcome`,
  `FetchOutcome`, `FetchProgress`, `FetchPhase`, `FetchError`, `RootfsFormat`)
  and `HardeningCapabilities` (already in types.rs, moves). The LOGIC stays; ziee
  re-imports the vocab from the engine. `runtime_mount::ensure_rootfs_ready` +
  `cache_dir` + `is_flavor_cached` currently take `&CodeSandboxState` and read
  `state.pool` (@228,@424) / `runtime_fetch::fetch_flavor_format` reads
  `state.pool` (@164) — refactor these to take the explicit fields (pool +
  host_caps + config + cache root) instead of `&CodeSandboxState`, since state
  loses `pool`. All 5 sites are ziee-side (Linux-verifiable).

### SEAMS (`seam.rs`, engine-defined; ziee-impl'd)

```rust
// B — relocate the pure fs chmod into the engine (was handlers::apply_workspace_mode).
pub async fn apply_workspace_mode(workspace: &std::path::Path) { /* linux 0o700 / mac 0o1777 / win noop, verbatim */ }

// D — guest-agent staging: a set-once app_data_dir global (cleanest; no signature
// churn on the un-verifiable mac/win arms — embedded::ensure()/wsl2_agent_embedded::ensure()
// stay arg-free). ziee mod.rs::init calls set_app_data_dir(get_app_data_dir()) at boot;
// embedded.rs/wsl2_agent_embedded.rs do_extract read app_data_dir() instead of
// crate::core::get_app_data_dir().  (Chosen over a GuestAgentStaging trait: the two
// do_extract bodies are the ONLY app_data consumers, both OnceCell get_or_try_init.)
pub fn set_app_data_dir(p: std::path::PathBuf); // OnceLock; ziee sets once at init
pub fn app_data_dir() -> std::path::PathBuf;     // engine reads

// ResourceLimitsProvider — only get()'s DB read is injected; snapshot_or_defaults()
// stays byte-identical DB-free in the moved resource_limits_cache.
#[async_trait] pub trait ResourceLimitsProvider: Send + Sync {
    async fn load_from_db(&self) -> Result<CodeSandboxResourceLimits, AppError>;
}
// engine free fn (moved get(), Repos call → provider): the 5 async sites
// (sandbox.rs:145, mcp_spawn.rs:350, mac_vm:505,733, wsl2:970) call
//   resource_limits_cache::get(state.limits.as_ref()).await?
// sync sites (mac_vm:121, wsl2:271,550) keep resource_limits_cache::snapshot_or_defaults() UNCHANGED.

// RootfsProvider — the backends call these instead of the STAY modules directly.
#[async_trait] pub trait RootfsProvider: Send + Sync {
    async fn ensure_rootfs_ready(&self, flavor: &str) -> Result<EnsureOutcome, AppError>; // linux_bwrap:39, sandbox.rs:125, mcp_spawn:317
    fn cache_dir(&self) -> std::path::PathBuf;                                            // mac_vm/wsl2
    async fn evict_by_version_flavor(&self, version_cache_dir: &std::path::Path, version: &str, flavor: &str) -> EvictOutcome; // default evict_artifact (mod.rs:161)
    async fn ensure_fetched(&self, cache_dir: &std::path::Path, flavor: &str, progress: Box<dyn Fn(FetchProgress)+Send+Sync>) -> Result<FetchOutcome, FetchError>;             // mac_vm:652,724,975
    async fn ensure_fetched_format(&self, cache_dir: &std::path::Path, flavor: &str, format: RootfsFormat, progress: Box<dyn Fn(FetchProgress)+Send+Sync>) -> Result<FetchOutcome, FetchError>; // wsl2:887,961,1236,1289
}
```
`register_mount` is NOT a provider method — it MOVES to the engine registry
(DB-free), so the mac/wsl `version_manager::register_mount` calls become
`ziee_sandbox::registry::register_mount`, and ziee's own `runtime_mount` call
site (@355) does likewise.

Coupling **C is MOOT**: explorers confirmed the VM backends do NOT read
`runtime_mount::READY` (only a stale doc comment at `mac_vm.rs:153`). No provider
accessor needed.

### `CodeSandboxState` (engine, de-`pool`-ed)
```rust
pub struct CodeSandboxState {
    pub config: CodeSandboxConfig,          // still crate::core::config (S-3 parked: optionally hoist to ziee-core + shim)
    pub loopback_url: String,
    pub workspace_root: PathBuf,
    pub host_caps: HostCapabilities,
    pub rootfs: Arc<dyn RootfsProvider>,    // was: pool
    pub limits: Arc<dyn ResourceLimitsProvider>,
}
```
ziee `mod.rs::init` builds a `ZieeRootfsProvider { pool, host_caps, config, cache_root }`
and `ZieeResourceLimitsProvider { pool }` (holding what the STAY logic needs),
then constructs state with the two Arcs. The provider impls delegate to the
STAY `runtime_mount`/`runtime_fetch`/`version_manager`/`resource_limits`
functions (refactored to explicit-field signatures).

`config.rs`'s `SandboxAvailability::PoolMissing` variant becomes vestigial (kept
for wire-compat or dropped — it's an internal enum surfaced via the versions
admin endpoint; if dropped, regen types.ts → NOT byte-identical golden, so KEEP
it, mapping the never-hit case).

### Order of operations for the move (all-or-nothing compile)
1. crate scaffold + Cargo.toml + lib.rs (mod decls).
2. `git mv` MOVE files; create seam.rs; move the vocab types + registry tail +
   limits struct.
3. Bulk sed in moved files: `crate::modules::code_sandbox::` → `crate::`;
   `crate::common::AppError` → `ziee_core::AppError`;
   `crate::core::config::CodeSandboxConfig` → keep (add path dep) or
   `ziee_core::config::…` if S-3 done; `crate::core::get_app_data_dir()` →
   `crate::seam::app_data_dir()` (embedded.rs/wsl2_agent_embedded.rs only).
4. Rewrite backend calls to STAY modules → provider methods (linux_bwrap +
   mac_vm + wsl2 + mod.rs default evict).
5. ziee: re-export shim in code_sandbox/mod.rs; provider impls; state build;
   refactor the 5 `&CodeSandboxState`-pool sites to explicit fields.
6. `cd sdk && cargo check --workspace`; `cargo check -p ziee`; `-p ziee-desktop`.
7. build-DB-free grep empty; golden regen byte-identical (handlers STAY → no
   OpenAPI delta; `git checkout --` after regen); tier-1 move+pass.
