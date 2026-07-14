# `ziee-sandbox` engine — coupling map on the CURRENT base (Phase-2 boundary)

Method: audited every `crate::` / cross-module reference in the engine-candidate
files (`sandbox.rs`, `cgroup.rs`, `probes.rs`, `config.rs`, `mcp_spawn.rs`,
`streaming.rs`, `mount_provider.rs`, `workflow_staging.rs`, `embedded.rs`,
`wsl2_agent_embedded.rs`, `tools/execute.rs`, the entire `backend/` tree).

## External `crate::` surface of the engine-candidate set (whole set)
```
 14  crate::common::AppError                 → ziee_core::AppError (re-export; substitute)
  7  crate::core::config::CodeSandboxConfig  → move struct to crate/ziee-core + app shim (parked S-3)
  2  crate::core::get_app_data_dir           → embedded.rs + wsl2_agent_embedded.rs (see item D)
  2  crate::modules::lit_search::fulltext::cache::{conversation_view_dir, SANDBOX_MOUNT_PATH}
```
The AppError + CodeSandboxConfig items match the parked design. The `lit_search`
+ `get_app_data_dir` items do NOT.

## Intra-module couplings the parked 14-substitution spec MISSED

### A. `/lit` reverse-dep inside the argv builder (Linux path — COMPILED here)
`sandbox.rs` (the `build_bwrap_argv` region, ~L809-825) pushes a `--ro-bind` for
`crate::modules::lit_search::fulltext::cache::conversation_view_dir(conv_id)` at
`SANDBOX_MOUNT_PATH` (`/lit`). This is inside the function that MUST move
byte-for-byte. A build-DB-free SDK crate cannot name `crate::modules::lit_search`.
Resolution: lift the `/lit` bind computation to the caller (ziee execute /
mount-context path) and pass it in as one more entry of the existing
`extra_ro_binds` — the bind lands in `all_ro_binds` at the SAME position, so the
emitted argv is byte-identical for a fixed input, but the builder SOURCE changes.

### B. `tools/execute.rs` → `handlers::apply_workspace_mode` (all platforms)
`apply_workspace_mode(&Path)` is a pure per-OS chmod (0700 linux / 1777 macos /
noop windows), DB-free. Relocate it into the moved execute path or a crate util.

### C. `mac_vm.rs` reads `runtime_mount::READY` directly (macOS arm — NOT compiled here)
Not only `ensure_rootfs_ready`; mac_vm dereferences the `READY` per-flavor caps
OnceCell global. The seam exposes `ensure_ready` (returns caps) but no
"snapshot the already-primed caps" accessor. Needs a provider method or a state
field.

### D. VM backends → embedded guest-agent staging (macOS+windows arms — NOT compiled here)
- `mac_vm.rs` → `embedded::ensure` (stage the `ziee-sandbox-agent` binary),
  `version_manager::register_mount`, `runtime_fetch::ensure_fetched`.
- `wsl2.rs` → `embedded::ensure`, `wsl2_agent_embedded::ensure`,
  `helper_service::client`, `version_manager::register_mount`,
  `runtime_fetch::ensure_fetched_format`.
`embedded.rs` + `wsl2_agent_embedded.rs` use `crate::core::get_app_data_dir`.
Guest-agent binary provisioning is a THIRD concern the two-seam design never
modeled. Options: (1) a 3rd `GuestAgentProvider` seam, or (2) move
`embedded.rs`/`wsl2_agent_embedded.rs`/`helper_service` WITH the backends and
thread `app_data_dir` through `CodeSandboxConfig`/state. `helper_service/` is a
`backend/` submodule and moves with the backend tree regardless.

### E. `streaming.rs` (SSE fetch-progress) — reclassify STAY
Uses `runtime_mount::is_flavor_cached`, `runtime_fetch::is_fetch_in_flight`, and
`runtime_fetch::ensure_fetched(cache, flavor, move |p| …)` with a REAL progress
callback that emits SSE frames. None are in the seam. `streaming.rs` is app UI
plumbing (0 DB queries but tightly bound to the fetch internals) → STAYS in ziee,
keeps calling ziee's `runtime_fetch`/`runtime_mount` directly. Parked CUT listed
it as moved — deviation.

## `version_manager.rs` split (91 KB) — feasible but delicate
DB-coupled surface (`query!`/`query_as!`/`Repos`): ~10 sites, all in the DB
registry region (~L1-1370: pins/artifacts/list_releases GitHub). The in-memory
registry + fs helpers to MOVE are localized in the tail:
`InflightKind`@1374, `InflightGuard`@1382, `register_mount`@1427,
`acquire_inflight`@1456, `wipe_install_caches_for_conversation`@1816,
`consume_workspace_sentinel`@1853. The split is doable (DB part STAYS, the tail's
TYPES move to the seam vocabulary, ziee re-exports), matching parked S-4, but is
real surgery on a large file.

## Files that would MOVE vs STAY (revised)
MOVE → `ziee-sandbox`: `caps`, `limits` (struct+defaults+FromRow-behind-feature+
provider), `seam`, `state`, `CodeSandboxConfig`, `cgroup.rs`, `probes.rs`,
`sandbox.rs` (lit-lifted), `tools/execute.rs` (+apply_workspace_mode),
`mcp_spawn.rs`, `mount_provider.rs`, `workflow_staging.rs`, the `backend/` tree
(linux_bwrap, mac_vm, wsl2, vm_client, vm_long_lived, hvsocket, unsupported,
helper_service/**), and — per the guest-agent decision — possibly `embedded.rs` +
`wsl2_agent_embedded.rs`.

STAY (ziee): `version_manager` (DB registry half + re-export moved tail),
`runtime_mount`, `runtime_fetch`, `resource_limits_cache` (=`ResourceLimitsProvider`
impl), `streaming.rs`, `handlers.rs`, `routes.rs`, `version_handlers.rs`,
`version_install_tasks.rs`, `repository.rs`, `models.rs`, `permissions.rs`,
`mount_context_extension.rs`, `version_back.rs`, `tools/files.rs`, `mod.rs`
(wires the two/three provider impls at boot), the `migrations/`, and the 5
domain-integration files. `code_sandbox/mod.rs` re-exports the moved engine
(`pub use ziee_sandbox::{…}`) so retained files' `super::`/`crate::modules::
code_sandbox::…` paths resolve unchanged (the ziee-hardware shim pattern).

---

## Phase-2 EXECUTED this session + design resolution

- **Coupling A (SECURITY-CRITICAL) — RESOLVED + VERIFIED.** The `/lit` reverse-dep
  was lifted out of `build_bwrap_argv` into a caller-injected
  `SandboxContext::extra_ro_binds`. `cargo check -p ziee`=0; new audit test
  `sandbox::tests::lit_bind_lift_is_byte_identical` proves the emitted argv is
  byte-for-byte identical apart from the injected bind triple (23/23 sandbox
  tier-1 tests pass). See TRANSFORMS.md.
- **Couplings B, D, E — design resolved** (apply_workspace_mode relocation;
  guest-agent staging via a set-once engine `app_data_dir` global — cleanest, no
  churn on the un-verifiable mac/win arms; streaming.rs STAY). See TRANSFORMS.md.
- **Coupling C — MOOT.** The VM backends do NOT read `runtime_mount::READY`
  (confirmed: only a stale doc comment at `mac_vm.rs:153`). No provider accessor
  needed. The original BOUNDARY item C over-modeled this.

## mac / windows RUNTIME verify commands (for the human, once the crate move lands)

The `mac_vm` (`cfg(macos)`) + `wsl2` (`cfg(windows)`) arms are `#[cfg]`-out on
Linux → compile-gated here, runtime-verified on the human's hosts.

**macOS (Apple Silicon; libkrun bundled at `Contents/Frameworks/libkrun.dylib`):**
```bash
# 1. compile the mac_vm arm (proves the seam rewrite didn't break the mac build):
cd src-app/server && cargo check -p ziee && cargo check -p ziee-desktop
cd sdk && cargo check --workspace
# 2. build a macOS rootfs + exercise the VM backend end-to-end (tier4 bwrap-direct
#    + tier6 HTTP-E2E — the only tiers that spawn the libkrun VM + guest agent):
just sandbox-build minimal && just sandbox-mount
ZIEE_SANDBOX_ROOTFS=.ziee-cache/sandbox-rootfs/current \
  cargo test --test integration_tests -- --test-threads=1 \
  --ignored code_sandbox::tier4_ code_sandbox::tier6_
# 3. sanity: run a real execute_command via the app and confirm the /lit carry-
#    forward + resource-limits provider + register_mount seam behave (fetch_info,
#    a python one-liner, evict).
```

**Windows (WSL2 ≥ 2.5.10/2.6.1; Docker pgvector DB on :54321):**
```powershell
# 1. compile the wsl2 arm:
cd src-app\server; cargo check -p ziee; cargo check -p ziee-desktop
cd ..\..\sdk; cargo check --workspace
# 2. provision the distro + run tier4/tier6 (needs a rootfs; ZIEE_WSL_VM_ID or the
#    helper service registered — see CLAUDE.md "Windows one-time admin steps"):
$env:ZIEE_SANDBOX_ROOTFS=".ziee-cache\sandbox-rootfs\current"
cargo test --test integration_tests -- --test-threads=1 `
  --ignored code_sandbox::tier4_ code_sandbox::tier6_
```
Focus of the human's review: (a) the two do_extract bodies read the engine's
set-once `app_data_dir` (not a stale `crate::core::get_app_data_dir`); (b) the
`RootfsProvider::{ensure_fetched(_format),cache_dir,evict_by_version_flavor}` +
`ziee_sandbox::registry::register_mount` calls in `mac_vm`/`wsl2` resolve + behave;
(c) the `/lit` `extra_ro_binds` carry-forward onto the guest argv.
