# Consolidated Warning-Suppression Audit — server + desktop

Branch `batchfix/warning-audit` @ `origin/batchfix/grand-merge` (0d96585f).
Method: 8 parallel agents, each grep-verifying every `#[allow]`/`#![allow]` site in
its file slice against the WHOLE workspace (server/ + desktop/), specifically
checking platform `cfg`, trait/macro/serde/sqlx/linkme indirection, and test-only
use before ruling anything dead. Per-chunk logs kept out-of-tree.

Scope audited: **~341 suppression sites across 231 files** (the `#[allow]` backlog),
PLUS the 3 currently-emitted live warnings from the first pass.

---

## 0. Actions taken in this commit (batch 1)

A careful compile-gated pass acted on the audit. **Both crates now compile with zero
warnings** (`cargo check -p ziee -p ziee-desktop --all-targets`). Summary:

- **3 live warnings → kept, not deleted (all false-dead):** `SecretView::new`/
  `expose_secret` (deliberate secret primitive, test-exercised), `proxy::clear_cache`
  (`#[cfg(test)]` callers), `TestServer::data_dir` (used by server integration tests;
  dead only in the desktop test binary). Each silenced with a narrow, commented `#[allow]`.
- **21 genuinely-dead zero-caller items deleted** (superseded methods, unused DTOs,
  diagnostic accessors, a dead `#[cfg(test)]` seam): `BranchMessage`,
  `create_content_with_id`, `ProjectFile`, `ConversationSummarizationSettings`,
  `SetSystemDefaultRequest`, `BinaryManager::{new,download_and_register,
  get_binary_path_by_version}`, `McpScope::{conversation_id,project_id}`,
  `insert_for_test`, `workspace_provenance_exists`, `RootfsFormat::ext`, `CgroupScope::path`,
  `connection_count`, `delete_user_defaults`, `InflightGuard::{artifact_id,version}`.
- **2 agent "REMOVE-SAFE" verdicts overridden → KEPT:** `ControlCatalog::is_empty`
  (exists to satisfy clippy's `len_without_is_empty`), and `deregister_mounts_for_flavor`
  (see §Latent findings).
- **2 cascade reverts:** deleting `cache_dir()` orphaned `binaries_dir`, and deleting
  `probe_all` orphaned the `SeccompMode::NotLinked` variant (stock-build seccomp-off
  state) — both restored (the compile-gate caught these).
- **Untouched (correctly):** the 8 KEEP-PLATFORM items (Windows/macOS sandbox,
  `GpuBackend::Metal`, cross-crate `register_sandbox_mount_provider`) and the ~250
  legitimate suppressions.

## Latent findings (real bugs surfaced by the audit — NOT fixed here)

- **mount-leak — `code_sandbox::version_manager::deregister_mounts_for_flavor`**: has
  ZERO callers. Its doc says `runtime_mount::evict_flavor` calls it, but that function
  **does not exist** in the tree. Consequence: the wholesale flavor-eviction path (admin
  `DELETE /code-sandbox/environments/{flavor}`) never flushes `MOUNTED_ARTIFACTS`, so
  evicting a flavor across pinned versions **leaks stale registry entries until the next
  server restart** — the exact leak the function was written to prevent. Kept under
  `#[allow(dead_code)]` with a `FIXME(mount-leak)` at the site. Fix = wire it into the
  flavor-eviction handler (or delete both if that path is genuinely gone).

## 0b. Actions taken (batch 2 — B + GATE)

- **B (unused_imports/mut cleanup):** removed the 5 spurious `#[allow(unused_imports)]`
  in the module `permissions.rs` files (the `PermissionInfo` import was genuinely unused;
  `cargo fix` dropped it). **Kept** the other 5 as load-bearing: `core/mod.rs`,
  `mcp/client/mod.rs` (×3), `auth/mod.rs` are lib-target-unused but consumed by the
  **binary** target / re-exported at crate root for the **desktop crate + tests**
  (`ziee::hash_password`) — `cargo fix --lib` wrongly stripped them (E0425/E0433), so they
  were restored. **Kept** both `#[allow(unused_mut)]` router builders: their `router = …`
  reassignment is `#[cfg(debug_assertions)]`, so `mut` is unused in `--release` and the
  gate would break release without the allow.
- **GATE:** `[workspace.lints]` in `src-app/Cargo.toml` + `[lints] workspace = true` in
  all 9 members. Scoped to `unused_imports = "deny"` + `unused_mut = "deny"` +
  `dead_code = "warn"` + `clippy::too_many_arguments = "allow"`. **Deliberately NOT the
  whole `unused` group** — `unused = "deny"` pulls in `unused_variables`, which fires on
  **135 idiomatic axum permission-extractor params** (`auth: RequirePermissions<…>`, used
  only for their FromRequest side-effect; all previously warning-free). Denying the group
  would hard-break the build and force a 135-site `_auth` rename across ~25 handler files —
  out of scope. Scoping to imports+mut delivers the stated goal (those suppressions can't
  regrow) with zero blast radius. **Verified:** `cargo check --workspace --all-targets` =
  0 errors; ziee + ziee-desktop = 0 warnings. (2 pre-existing `dead_code` warnings remain
  in `sandbox-vm-launcher` + `ai-providers` test — non-breaking, Tier-C candidates.)
- **CI guard:** `scripts/check-deadcode-blankets.sh` + baseline
  `scripts/deadcode-blanket-baseline.txt` (the 90 files below), wired into `just check`.
  Denies NEW `#![allow(dead_code)]` module blankets; removals (Tier-C paydown) always pass.

---

## 1. Bottom line

**Almost nothing is blindly deletable.** Of ~341 suppressed sites, only **23** are
confidently REMOVE-SAFE (genuinely dead on every platform). The rest are load-bearing
for reasons a naive `cargo fix`/delete would break — and **8 are platform-gated items
that would compile-break Windows/macOS if removed.** This validates the concern: a
mass-fix would have introduced real bugs.

### Verdict totals (approximate; see per-chunk logs for exact rows)

| Verdict | ~Count | Meaning / action |
|---|---|---|
| **REMOVE-SAFE** | **23** | Zero callers on any platform → safe to delete the item. Still compile-verify as a batch. |
| **DROP-ALLOW-ONLY** | ~77 | Item IS used → the `#[allow]` is spurious; keep item, the allow can go (compile-verify). Many are whole-file blankets covering only `#[test]` fns. |
| **KEEP-FUTURE-API** | ~79 | Intentional pub/scaffolding API (repository/events/types), 0 callers today — deletion is a product judgment, not a warning fix. |
| **KEEP-TESTONLY** | ~63 | Used only by tests / shared test fixtures / harness; per-integration-test-crate warnings are expected. |
| **KEEP-TRAIT/MACRO/DERIVE** | ~48 | Used via serde/schemars/sqlx `FromRow`/linkme `#[distributed_slice]`/trait objects — rustc under-reports. Deleting breaks runtime wiring. |
| **KEEP-PLATFORM** | **8** | Dead on Linux, **LIVE on Windows/macOS**. Deleting = broken cross-platform build. See §2. |
| **KEEP-CLIPPY-style** | ~18 | `clippy::too_many_arguments` etc. on live fns; suppression is legitimate. |
| **INVESTIGATE** | ~14 | Needs an actual compile (allow removed) to settle field/variant/within-file status. See §3. |

---

## 2. ⚠️ KEEP-PLATFORM — the 8 items that would break non-Linux builds (DO NOT DELETE)

These look dead under Linux `cargo check` but are compiled + used on other targets:

| Item | File | Platform |
|---|---|---|
| `allow_wsl2_mirrored_mode` | `code_sandbox/version_manager.rs` (chunk00) | Windows (WSL2) |
| `vm_long_lived.rs` prod-path items (multiple) | `code_sandbox/backend/vm_long_lived.rs` (chunk01) | macOS (libkrun) + Windows (WSL2) |
| `register_sandbox_mount_provider` | (chunk01) | consumed cross-crate by `desktop/tauri` — invisible to a server-only check |
| `RootfsFormat::TarZst` | `code_sandbox/runtime_fetch.rs` (chunk01) | Windows (WSL2 import format) |
| `GpuBackend::Metal` | `llm_local_runtime/engine/gpu_detect.rs:168` (chunk03) | macOS |

**Implication:** any dead_code sweep MUST run on Linux **and** be cross-checked on the
Windows + macOS build hosts before deletion, or gated by `#[cfg(...)]` rather than deleted.

---

## 3. The 23 confident REMOVE-SAFE items (zero callers, all platforms)

Delete the item + its `#[allow]`. Compile-verify as one batch afterward.

**chunk01 (8):** `BranchMessage` (chat/core/models/message.rs:77); unused `PermissionInfo`
import (chat/core/permissions.rs:1); `create_content_with_id` (chat/core/repository/contents.rs:88);
`connection_count` (chat/stream registry.rs:238); cgroup `path()` (cgroup.rs:74);
`probe_all` (probes.rs:65); `workspace_provenance_exists` (repository.rs:89);
`RootfsFormat::ext()` (runtime_fetch.rs:81).

**chunk03 (5):** `BinaryManager::new` (binary_manager.rs:30, superseded by `with_cache_dir`);
`download_and_register` (binary_manager.rs:51); `get_binary_path_by_version` (:169);
`cache_dir()` (:412); `SetSystemDefaultRequest` (runtime_version/models.rs:47, unrouted).

**chunk02 (4):** `artifact_id()` + `version()` (version_manager.rs:1349/1354);
`deregister_mounts_for_flavor` (:1443); `is_empty()` (control_mcp/catalog.rs:75).

**chunk04 (4):** `delete_user_defaults` (mcp/chat_extension/defaults/repository.rs:88);
`insert_for_test` (mcp/client/manager.rs:298); `McpScope::conversation_id()` +
`project_id()` (mcp/settings/models.rs:25/32).

**chunk05 (2):** `ProjectFile` (project/models.rs:36); `ConversationSummarizationSettings`
(summarization/models.rs:55).

> Note: even these 23 should be deleted then compiled — a couple (e.g. `RootfsFormat::ext`,
> binary_manager methods) sit near platform code; the audit found no callers but a build is the
> final proof.

## 3 currently-emitted (live) warnings — separate from the suppressed set

All dead_code, none platform/trait: `SecretView::new`+`expose_secret` (common/secret.rs:38/45 —
verify it's not intended secret-wrapper API), `clear_cache` (llm_local_runtime/proxy.rs:105),
`TestServer::data_dir` (tests/common/harness_inner.rs:772, test-harness). These are the only
things making `cargo check` non-silent today.

---

## 4. INVESTIGATE — needs a compile to settle (~14)

Read-only grep can't see struct-field / enum-variant / within-file dead code that `dead_code`
also flags. These need the allow removed + one `cargo check`:
- **chunk06 workflow (5):** dispatch.rs:13, handlers/dev.rs:9, workflow_mcp/handlers.rs:16
  (`dispatch_resources_read`), progress_sse.rs:9 (`snapshot_manifest`), runner.rs:19 (cap consts).
- **chunk03 (3):** `TaskState::{Verifying,Extracting,Registering}` — download-lifecycle enum
  variants never constructed today (future-wired?).
- **chunk05 large repos NOT exhaustively verified:** `skill/repository.rs` (47 methods),
  `user/repository.rs` (32), `user/service.rs` — flagged for a dedicated per-item pass.
- **chunk06 caveat:** the ~20 DROP-ALLOW-ONLY workflow blankets appear to cover only `#[test]`
  fns (spurious allows), but weren't exhaustively checked for fields/variants → confirm by compile.

---

## 5. Recommended safe action sequence (when you decide to act)

1. **Delete the 23 REMOVE-SAFE items + 3 live-warning items**, then `cargo check -p ziee -p
   ziee-desktop --all-targets` on Linux. Green = done for this batch.
2. **DROP-ALLOW-ONLY sweep, per file, compile-gated:** remove the blanket/item allow, `cargo
   check`; if clean, keep removed; if it re-warns, the allow was load-bearing → restore as a
   *narrow, commented* per-item allow. This is the only way to safely convert the ~77 + settle
   the ~14 INVESTIGATE. Grindable module-by-module.
3. **KEEP-PLATFORM (8):** never delete on Linux alone — convert any spurious ones to
   `#[cfg_attr(target_os=..., allow(dead_code))]` and verify on the Windows + macOS hosts.
4. **KEEP-FUTURE-API / TRAIT / TESTONLY / CLIPPY:** leave as-is; optionally add a one-line
   reason comment on each allow so future audits don't re-litigate.
5. **Gate to stop regrowth:** `[workspace.lints]` with `unused="deny"`, `dead_code="warn"`;
   CI grep-ban NEW `#![allow(dead_code)]` module blankets (baseline the existing ~90).

**Nothing here should be mass-`cargo fix`'d.** The safe unit of work is one file → remove allow →
compile → keep-or-narrow, with the Windows/macOS hosts consulted for anything under `code_sandbox`,
`llm_local_runtime/gpu`, `auth/providers/apple`, or `desktop/`.

---

## 6. Tier C worklist — the 90 module-level `#![allow(dead_code)]` blankets

### Paydown progress (90 → 62 remaining)

- ✅ **workflow (27) + server/src core (1)** — CLEARED on `batchfix/tierc-live5`.
  Blankets removed; `cargo check -p ziee --tests` clean. One genuinely-dead item
  DELETED (`validate::cycle_check`, zero callers, superseded by `topo_sort_steps`);
  the rest were legit future/test/scaffolding API and kept under narrow, commented
  per-item `#[allow(dead_code)]` (repository facade, SSE/event emitters, REST-stream
  file/log/artifact helpers, type predicates, the core `AppEvent` aggregator enum,
  serde default fn, reserved `ConversationWorkflowOverride`, `PER_CALL_TOKEN_CAP`
  FIXME).

These whole-file suppressions are the backlog to pay down incrementally (convert
each to per-item `#[allow(dead_code)]`-with-reason or delete the dead item; a
compile after removing each blanket surfaces the real per-item warnings). The
workspace gate keeps `dead_code = "warn"` so this stays non-breaking, and
`scripts/check-deadcode-blankets.sh` (baseline `scripts/deadcode-blanket-baseline.txt`)
denies NEW blankets while allowing removals. Grouped by module for grind-fleet
partitioning (counts in parens):

| Module | # | Files |
|---|---|---|
| assistant | 2 | assistant/chat_extension/extension.rs assistant/events.rs  |
| auth | 4 | auth/events.rs auth/jwt_extractor.rs auth/providers/ldap.rs auth/providers/local.rs  |
| chat | 9 | chat/core/extension/registry.rs chat/core/extension/request.rs chat/core/models/content.rs chat/core/models/message.rs chat/core/services/streaming.rs chat/core/types/conversation.rs chat/core/types/message.rs chat/core/types/streaming.rs chat/extensions/title/extension.rs  |
| citations | 1 | citations/chat_extension/extension.rs  |
| code_sandbox | 1 | code_sandbox/mount_context_extension.rs  |
| control_mcp | 1 | control_mcp/chat_extension/extension.rs  |
| file | 1 | file/project_extension/events.rs  |
| hub | 4 | hub/events.rs hub/models.rs hub/repository.rs hub/types.rs  |
| lit_search | 1 | lit_search/chat_extension/extension.rs  |
| llm_local_runtime | 1 | llm_local_runtime/events.rs  |
| llm_model | 4 | llm_model/events.rs llm_model/models.rs llm_model/repository.rs llm_model/storage.rs  |
| llm_provider | 3 | llm_provider/events.rs llm_provider/repositories/admin.rs llm_provider/user_extension/repository.rs  |
| llm_repository | 1 | llm_repository/events.rs  |
| mcp | 4 | mcp/chat_extension/content.rs mcp/events.rs mcp/repository.rs mcp/types.rs  |
| memory | 1 | memory/chat_extension/extension.rs  |
| permissions | 2 | permissions/extractors.rs permissions/types.rs  |
| project | 3 | project/chat_extension/extension.rs project/core/extension/registry.rs project/events.rs  |
| skill | 6 | skill/chat_extension/extension.rs skill/dev_handlers.rs skill/events.rs skill/models.rs skill/repository.rs skill/types.rs  |
| skill_mcp | 3 | skill_mcp/file_cache.rs skill_mcp/handlers.rs skill_mcp/tools.rs  |
| summarization | 1 | summarization/chat_extension/extension.rs  |
| user | 5 | user/events.rs user/models.rs user/permissions.rs user/repository.rs user/service.rs  |
| web_search | 1 | web_search/chat_extension/extension.rs  |
| workflow ✅ | 27 | workflow/artifact_io.rs workflow/artifact_stream.rs workflow/compiled.rs workflow/cost.rs workflow/dispatch.rs workflow/elicit.rs workflow/events.rs workflow/file_io.rs workflow/handlers/dev.rs workflow/handlers/mod.rs workflow/handlers/system.rs workflow/log_io.rs workflow/log_stream.rs workflow/models.rs workflow/output_stream.rs workflow/progress_sse.rs workflow/ref_check.rs workflow/registry.rs workflow/repository.rs workflow/routes.rs workflow/runner.rs workflow/startup_sweep.rs workflow/template.rs workflow/test_runner.rs workflow/type_infer.rs workflow/types.rs workflow/validate.rs  |
| workflow_mcp | 3 | workflow_mcp/handlers.rs workflow_mcp/resources.rs workflow_mcp/tools.rs  |
| server/src (core) ✅ | 1 | core/events.rs  |

Full authoritative list: `scripts/deadcode-blanket-baseline.txt` (90 paths).
