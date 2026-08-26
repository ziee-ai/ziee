# PLAN — upstream-port

Carry paws' genuinely-shared-code defect fixes up to `ziee-ai/ziee`, hand-written
against **upstream's** shape rather than replayed as paws commits. Other ziee
instances consume this repo, so the diff stays as narrow as the defects require and
carries nothing of paws' product direction.

## Design source

This is a PORT. The "design" for each item is the paws commit that authored the fix
plus the task brief that commissioned it; nothing new is invented here.

- Realizes paws `232c92bbb` (`fix(desktop): make ziee-desktop compile in a release build`).
- Realizes paws `816aa6321` §"MAKE THE KILL SWITCHES REAL" and its `app_builder`
  postscript, and the design it audits, `docs/design/paws-feature-surface.md`.
- Realizes paws PR #12 (`fix/realtime-sse-delivery`, merge `bbea33a08`) and its brief
  `/data/khoi/home-workspace/paws-worker-tasks/realtime-sse.md`, restricted to the
  **sdk-free** half — see `## Out of scope`.
- Realizes paws `b973f6545` (`fix(desktop): run beforeBuildCommand from the repo root`).
- Realizes paws `4794cb754`, `1d06d4b17`, `2cd837ced` (pgvector macOS + build diagnostics).
- Realizes paws `28a80dc41` (LFS progress) and the keep-alive/wire-shape half of PR #12.
- Scope boundary is the worker brief
  `/data/khoi/home-workspace/paws-worker-tasks/upstream-sync.md` §"(A) PUSH UP",
  including its hard exclusion list.

## Invariants

Lifted VERBATIM from the authoring commits / briefs.

- **INV-1**: `ziee-desktop` did not compile
- **INV-2**: register_routes now guards, per CLAUDE.md 16
- **INV-3**: voice/js_tool/web_search/lit_search module structs now default enabled=false so route registration fails CLOSED if init never ran
- **INV-4**: The settings/admin REST is split out and stays mounted, because web_search and lit_search are disable-ONLY rows whose admin pages the design keeps
- **INV-5**: a custom request header the API reads must be accepted by the API's own CORS preflight
- **INV-6**: Both are realtime delivery to the UI failing while the underlying operation succeeds
- **INV-7**: app_builder's test sorted its expectation by order alone while create_modules sorts by (order, name), so the expectation was linker-dependent

## Items

- **ITEM-1**: `enable_popout_windows: false` on the desktop `WindowConfig` literal, and `#[cfg(debug_assertions)]` on the `axum::{body::Body, http::Request, response::Response}` import whose only consumer is the debug-only `proxy_to_vite`. (INV-1)
- **ITEM-2**: Extract the desktop's inline CORS literal into a testable free function and add `X-Chat-Stream-Connection-Id` to its `allow_headers`; add the same header to `dev.example.yaml` and `prod.example.yaml`; make `CHAT_STREAM_CONNECTION_HEADER` `pub` so the server side can be referenced rather than re-spelled. (INV-5)
- **ITEM-3**: Add the `enabled` field + fail-closed default + `register_routes` guard to `js_tool`, `web_search` and `lit_search`, splitting each module's router so the settings/admin REST stays mounted and only the MCP JSON-RPC endpoint is gated. Upstream's `unwrap_or(true)` defaults are left untouched. (INV-2, INV-3, INV-4)
- **ITEM-4**: Rebuild `progress_data` at the download-SSE consumer instead of spreading the flat wire event over the row, and drop the `as DownloadInstance` cast that hid the mismatch from `tsc`. (INV-6)
- **ITEM-5**: Own both ends of the LFS progress channel in a `spawn_forwarder`, replacing the orphaned `_lfs_progress_rx` binding that both froze the bar and queued every per-chunk send forever. (INV-6)
- **ITEM-6**: `.keep_alive(KeepAlive::default())` on the download SSE response, plus the wire-shape tests that pin the flat event and its null-vs-zero semantics. (INV-6)
- **ITEM-7**: `MACOSX_DEPLOYMENT_TARGET=11.0` for the pgvector make invocation, and surface make's output + the failure reason through `cargo:warning=` instead of `eprintln!`, which cargo swallows.
- **ITEM-8**: `beforeBuildCommand` runs the npm workspaces from the repo root rather than `cd`-ing relative to the tauri dir.
- **ITEM-9**: Break the `create_modules_instantiates_all_entries_in_order` expectation's tie by `(order, name)` so it stops depending on linker order. (INV-7)
- **ITEM-10**: Assert the branch moves **no** submodule gitlink and touches no paws-product surface.
- **ITEM-11**: Repair the two unit tests that are RED on `upstream/main` today, found while verifying this port (see BASE.md for the measurement). `llm_repository/utils.rs`'s `capability_url_targets_the_kinds_listing_surface` asserts a probe URL `beae7c7fb` deliberately stopped producing; `background_mcp/tools.rs`'s missing-`spec` refusal hands back a spec-level example where its own comment and its own test require a full arguments object. **Not a paws port** — an upstream defect this branch discovered; kept as its own commit so it can be dropped independently of the ports.

## Out of scope — recorded so the omissions are deliberate, not silent

- **The GPU/CUDA detection fix.** Upstream still has the defect (its `gpu_detect.rs`
  scrapes the literal `CUDA Version:`, 4 occurrences, zero `CUDA UMD` handling; and
  its sdk's `ziee-hardware/src/detection.rs:201` has the same scrape). But paws' fix
  moved the parser into a NEW sdk module, `crates/ziee-hardware/src/gpu_version.rs`,
  which does not exist on the sdk line upstream pins. Porting it needs an sdk change,
  which the brief reserves for the owner. Escalated, not ported; a separate sdk PR
  carries the sdk half.
- **The CORS `create_cors_layer_with` union.** Same shape: `FRAMEWORK_REQUIRED_REQUEST_HEADERS`
  and `create_cors_layer_with` exist only on paws' sdk line. ITEM-2 is the sdk-free
  subset that fixes the actual user-visible bug; the union is defence-in-depth and
  goes with the sdk PR.
- **The macOS ggml `.so`→`.dylib` symlink shim** (`a9ab79375`) — a workaround for a
  defect in the `ziee-ai/llama.cpp` RELEASE BUILD, and unverifiable without a Darwin
  toolchain. Owner decision: report it, do not port it.
- The four permanently-excluded buckets: `stores/` case-collision renames, the
  `feat/paws-feature-surface` reduction itself (only its shared-code kill-switch
  GUARD is taken, never paws' default flips), the tinnlab HF mirror + default-model
  onboarding, and every sdk pointer move. Plus paws' CI, updater endpoint/pubkey, and
  desktop README.

## Files to touch

- `src-app/desktop/tauri/src/modules/backend/mod.rs` (ITEM-1, ITEM-2)
- `src-app/desktop/tauri/Cargo.toml` (ITEM-2 — `tower` dev-dep for the preflight test)
- `src-app/desktop/tauri/tauri.conf.json` (ITEM-8 — the `beforeBuildCommand` line ONLY)
- `src-app/server/src/modules/chat/stream/handler.rs` (ITEM-2 — `pub` on the constant)
- `src-app/server/config/{dev.example.yaml,prod.example.yaml}` (ITEM-2)
- `src-app/server/src/modules/{js_tool,web_search,lit_search}/{mod.rs,routes.rs}` (ITEM-3)
- `src-app/ui/src/modules/llm-provider/stores/llmModelDownload/actions/subscribeToDownloadProgress.ts` + `…/subscribeToDownloadProgress.store.test.ts` (ITEM-4)
- `src-app/server/src/modules/llm_model/handlers/{lfs_progress.rs (new),uploads.rs,mod.rs,downloads.rs}` (ITEM-5, ITEM-6)
- `src-app/server/tests/llm_model/download_stream_keepalive_test.rs` (new, ITEM-6)
- `src-app/server/build.rs`, `src-app/server/build_helper/pgvector.rs` (ITEM-7)
- `src-app/server/src/core/app_builder.rs` (ITEM-9)

## Patterns to follow

- ITEM-3's reference implementation is **upstream's own `VoiceModule`**, which already
  caches `enabled` at `init()` and guards `register_routes`. Mirror it exactly rather
  than inventing a third shape.
- ITEM-2's reference is the `X-Sync-Connection-Id` entry immediately above it in the
  same `allow_headers` vec — including its comment style explaining the silent-failure
  mode, which is what made its sibling's absence survive so long.
- ITEM-5/ITEM-6's reference is any other SSE route in the tree (all of which already
  set `keep_alive`).
- Everywhere: keep upstream's existing defaults, comments and error codes. A hunk that
  only exists because paws defaults differently is out of scope by construction.

## Item verdicts (phase 2)

Below, in this file's `# PLAN AUDIT` section.

---

# PLAN AUDIT (phase 2) — audited against the UPSTREAM tree

## Breakage risk

The two items with real breakage risk are ITEM-3 and ITEM-2, and both were checked
against upstream's own code rather than paws':

- **ITEM-3** changes when a router is merged. The risk is un-mounting something a
  deployment depends on. Mitigated by the split: only the MCP JSON-RPC endpoint is
  gated; the settings/admin REST always mounts, so the admin pages keep working on a
  disabled deployment. And upstream's defaults are untouched (`unwrap_or(true)`), so
  for every deployment that has not set the kill switch, `self.enabled` is `true` and
  the routing is byte-identical to before.
- **ITEM-2** adds a `pub` + a crate-root re-export. Additive; no existing caller
  changes. The desktop test needs a `tower` dev-dep, which is already in upstream's
  `[workspace.dependencies]` (`src-app/Cargo.toml:117`), so the `Cargo.lock` delta is
  exactly one line.
- **ITEM-1**'s `#[cfg(debug_assertions)]` on the axum import is safe because
  `proxy_to_vite`, the only consumer, already carries the identical gate.
- **ITEM-5** replaces an orphaned channel receiver with an owned forwarder and awaits
  it before any terminal write. Risk would be a hang if the forwarder never ended; it
  ends when the pull drops its sender, which it does on return.

## Pattern conformance

ITEM-3 mirrors upstream's own `VoiceModule`, which already caches `enabled` at
`init()` and guards `register_routes()` — so this makes three modules match a shape
upstream already chose, rather than introducing a fourth. ITEM-6's `keep_alive` makes
the download SSE route match every other SSE route in upstream's tree. ITEM-2's new
allow_headers entry sits directly beneath its sibling `X-Sync-Connection-Id` and
copies its comment style, because that sibling's comment describing this exact
failure mode is what makes the omission obvious to the next reader.

## Migration collisions

None — this branch adds no migration. See BASE.md.

## OpenAPI regen

Not implied. See BASE.md. ITEM-3 changes only whether a router is merged, not the
shape of any route it declares.

## Item verdicts

- **ITEM-1** — verdict: PASS — verified RED first: `cargo check -p ziee-desktop` on
  untouched `upstream/main` fails with `error[E0063]: missing field
  `enable_popout_windows``. Both hunks are 1 line + comment.
- **ITEM-2** — verdict: PASS — verified the defect is live upstream: the header is
  READ at `chat/stream/handler.rs:206` and appears in NO allowlist
  (`git grep` over `desktop/.../mod.rs` and both example configs).
- **ITEM-3** — verdict: PASS — verified the hole by reading upstream's own
  `js_tool/mod.rs`: `init()` early-returns on the kill switch while
  `register_routes` merges unconditionally, and migration
  `202607146040_js_tool_grant_permissions.sql` grants `js_tool::use` to the Users
  group. So "disabled" leaves arbitrary QuickJS execution reachable by any user.
- **ITEM-4** — verdict: PASS — upstream still has `{ ...download, ...update } as
  DownloadInstance`; the cast is what hid the flat-vs-nested mismatch from `tsc`.
- **ITEM-5** — verdict: PASS — upstream still binds `_lfs_progress_rx`
  (`uploads.rs:1261`). Note the binding is underscore-PREFIXED, not `_`, so the
  receiver lives to end of scope and every send SUCCEEDS and queues — a leak on top
  of the frozen bar.
- **ITEM-6** — verdict: PASS — `git grep keep_alive` over upstream's `downloads.rs`
  returns nothing; every other SSE route in the tree sets it.
- **ITEM-7** — verdict: CONCERN — the `MACOSX_DEPLOYMENT_TARGET` half is
  **unverifiable on this box** (no Darwin toolchain, and the brief forbids macOS
  builds). It sits inside the existing `target.contains("apple")` branch, so it
  cannot affect the Linux or Windows paths, and both repos pin
  `POSTGRESQL_VERSION = "=18.3.0"` so the premise holds. Shipping with the limitation
  stated in the PR body rather than silently. The `cargo:warning=` half is verified.
- **ITEM-8** — verdict: PASS — one line; the sibling updater hunk in the same file is
  paws-specific and is NOT taken.
- **ITEM-9** — verdict: PASS — `sort_by_key` is stable, so ties keep linker order
  while `create_modules` breaks them by name; matching the implementation's key is
  the fix.
- **ITEM-10** — verdict: PASS — asserted post-hoc; `git status` shows no submodule
  gitlink change.
- **ITEM-11** — verdict: CONCERN — added AFTER the plan, from an audit finding, and
  it is not a paws port at all but a repair of two tests that are RED on
  `upstream/main` today. Kept as its own commit so it can be dropped independently.
  Not BLOCKED: it is small, verified RED-then-GREEN, and leaving a repo's own test
  suite red when the fix is known would be the worse call.
