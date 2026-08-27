# TESTS — upstream-port

**Honest deviation, stated up front.** This is a PORT of fixes that already shipped
in paws, so most tests below are the ones authored WITH the original fix, re-run in
upstream's tree. Where paws' version of a test depended on paws-only scaffolding it
was rewritten against upstream's shape rather than dragging that scaffolding along
(the brief requires exactly this). Every TEST names a real test that exists on this
branch.

**`ziee-ai/ziee` has NO PR CI** — two tag-triggered release workflows and nothing
else. So this list is not a supplement to an automated gate; it IS the gate, and it
is why several entries below are `verified RED first` rather than merely `passes`.

The diff touches `src-app/ui/**` (ITEM-4), so the phase-3 frontend rule applies. See
TEST-13 for how it is answered, honestly, rather than with a hollow spec.

## ITEM-1 — the desktop crate compiles (INV-1)

- **TEST-1** (tier: unit) [acceptance] [invariant: INV-1] [covers: ITEM-1] file: `src-app/desktop/tauri/src/modules/backend/mod.rs` — asserts: that `ziee-desktop` COMPILES AT ALL. The proof is the compiler, not an assertion: `cargo check -p ziee-desktop` on untouched `upstream/main` fails `error[E0063]: missing field enable_popout_windows`, and passes with ITEM-1. Recorded as a command + its captured output in TEST_RESULTS.md, because "the crate builds" is not expressible as a `#[test]` inside that crate — a test file cannot run if the crate it lives in does not compile.
- **TEST-2** (tier: unit) [covers: ITEM-1] file: `src-app/desktop/tauri/src/modules/backend/mod.rs` — asserts: `desktop_boot_captures_the_bound_server_addr_not_the_default` — the pre-existing sibling test still passes, i.e. ITEM-1's edits to the same file did not disturb the boot path.

## ITEM-2 — the chat-stream header reaches the preflight (INV-5)

- **TEST-3** (tier: unit) [acceptance] [invariant: INV-5] [covers: ITEM-2] file: `src-app/desktop/tauri/src/modules/backend/mod.rs` — asserts: `desktop_cors_allows_the_chat_stream_subscription_header` — drives the REAL tower CORS layer with a real `OPTIONS` preflight and asserts `Access-Control-Allow-Headers` CONTAINS `x-chat-stream-connection-id`. Asserted through the service rather than by inspecting the `Vec<String>` because what a browser obeys is the response header, which only the layer produces. **Verified RED by mutation**: with the entry removed it fails `got "authorization,content-type,accept,origin,x-sync-connection-id"` — byte-identical to what a live upstream desktop instance returns.
- **TEST-4** (tier: unit) [covers: ITEM-2] file: `src-app/desktop/tauri/src/modules/backend/mod.rs` — asserts: `desktop_cors_config_lists_both_connection_id_headers` — the list is sourced from `ziee::CHAT_STREAM_CONNECTION_HEADER`, the handler's own constant, so a rename on the server side turns this red instead of silently un-allowing the header. Also verified RED by the same mutation.

## ITEM-3 — the kill switches actually disable (INV-2, INV-3, INV-4)

- **TEST-5** (tier: unit) [acceptance] [invariant: INV-3] [covers: ITEM-3] file: `src-app/server/src/modules/js_tool/mod.rs` — asserts: `config_default_enabled` — upstream's existing test, unchanged, proving ITEM-3 did NOT alter the config default. This is the control that distinguishes "the guard was added" from "paws' default flip was smuggled in", which is the specific thing this port must not do.
- **TEST-6** (tier: unit) [covers: ITEM-3] file: `src-app/server/src/modules/js_tool/mod.rs` — asserts: `server_id_is_stable_and_distinct` + `module_present` — the module still registers and its built-in id is unchanged.
- **TEST-7** (tier: unit) [covers: ITEM-3] file: `src-app/server/src/core/app_builder.rs` — asserts: `create_modules_instantiates_all_entries_in_order` — every module, including the three whose structs gained a field, still constructs exactly once and in order. This is what catches a broken `new()`.
- **TEST-19** (tier: integration) [acceptance] [invariant: INV-2] [covers: ITEM-3] file: `src-app/server/tests/mcp/kill_switch_gate_test.rs` — asserts: with each module disabled its MCP JSON-RPC route is 404 — UNMOUNTED, not merely refused. The caller is in the default group and holds the module's `use` grant, so a mounted route would SERVE it (200); that is what makes 404 mean absent. Plus a positive control that DEFAULTS leave every endpoint mounted, without which the rest passes vacuously.
- **TEST-20** (tier: integration) [acceptance] [invariant: INV-4] [covers: ITEM-3] file: `src-app/server/tests/mcp/kill_switch_gate_test.rs` — asserts: with each module disabled its settings/admin REST is NOT 404 — the deliberate split. A 404-only assertion would pass against a guard that unmounted the whole router, which is exactly the over-reach round 1 found and fixed in js_tool.
- **TEST-8** (tier: unit) [covers: ITEM-3] file: `src-app/server/src/modules/web_search/mod.rs`, `src-app/server/src/modules/lit_search/mod.rs` — asserts: the existing in-module unit tests for both modules still pass after the struct + router split.


## ITEM-4/5/6 — realtime download progress (INV-6)

- **TEST-9** (tier: unit) [acceptance] [invariant: INV-6] [covers: ITEM-4] file: `src-app/ui/src/modules/llm-provider/stores/llmModelDownload/subscribeToDownloadProgress.store.test.ts` — asserts: what a VIEW would render advances across successive SSE frames — `progress_data.current`/`total`/`speed_bps` are rebuilt, not left at the REST snapshot's zeros. Asserts the render, not the server's write; the previous attempt asserted the write and the write was never the broken half.
- **TEST-10** (tier: unit) [covers: ITEM-4] file: `…/subscribeToDownloadProgress.store.test.ts` — asserts: a `null` FIGURE keeps the value already on screen (null means "unknown", never "zero"), while a `null` on a WHOLE-ROW field (`error_message`, `model_id`) is taken as a genuine clear — the two have opposite semantics and getting it backwards left stale red error text on a cleared row.
- **TEST-11** (tier: unit) [acceptance] [invariant: INV-6] [covers: ITEM-5, ITEM-6] file: `src-app/server/src/modules/llm_model/handlers/downloads.rs` — asserts: `wire_shape_tests` — the SSE payload stays FLAT (a nested `progress_data` would silently re-break every consumer), and an absent progress row serialises the figures as `null` rather than `0`. This is the server-side half of the contract TEST-9/10 consume.
- **TEST-12** (tier: integration) [acceptance] [invariant: INV-6] [covers: ITEM-6] file: `src-app/server/tests/llm_model/download_stream_keepalive_test.rs` — asserts: `download_progress_stream_sends_keepalives_while_idle` — reads the actual wire for ~16s and requires an SSE comment frame. Asserted by reading bytes rather than by checking the builder was called, because `keep_alive(...)` not being wired to the response is exactly the failure it must catch; a closed stream fails the test explicitly.
- **TEST-13** (tier: e2e) [covers: ITEM-4] file: `src-app/ui/tests/e2e/12-local-runtime/download-progress.spec.ts` — asserts: a running download's rendered byte counts and percentage advance in the UI. **NOT VERIFIED.** The phase-3 rule demands an e2e for a UI-touching diff. The honest position: ITEM-4 is a pure store-action change with no new surface, its behaviour is fully pinned by TEST-9/10 against the real store, and driving a multi-GB download through Playwright to watch a progress bar is not something this box can do. Writing a spec that merely opens the page would satisfy the gate and prove nothing — the hollow-test failure this process exists to prevent. Recorded as NOT VERIFIED with the reason, which the gate treats as a failure rather than a pass, and called out in the PR body.

## ITEM-7/8/9 — build correctness

- **TEST-14** (tier: unit) [acceptance] [invariant: INV-7] [covers: ITEM-9] file: `src-app/server/src/core/app_builder.rs` — asserts: `create_modules_instantiates_all_entries_in_order` with the `(order, name)` key — the expectation no longer depends on linker order. Same test as TEST-7, cited here for the different property it now pins.
- **TEST-15** (tier: unit) [covers: ITEM-7, ITEM-8] file: `src-app/server/build_helper/pgvector.rs` — asserts: a pgvector build failure surfaces through `cargo:warning=` rather than swallowed stderr, and the macOS deployment target is raised so PG18 headers compile. **PARTIALLY VERIFIED.** The `cargo:warning=` conversion is exercised on every build of this branch (the helper runs). The `MACOSX_DEPLOYMENT_TARGET` line sits inside `target.contains("apple")` and is **unreachable on Linux**, so it is NOT verified here — no Darwin toolchain, and the brief forbids macOS builds. Likewise `beforeBuildCommand` is only executed by a real `tauri build`. Both are recorded as not-verified-on-this-platform rather than claimed.

## ITEM-10/11 — hygiene and the upstream repair

- **TEST-16** (tier: unit) [covers: ITEM-10] file: `.lifecycle/upstream-port/TEST_RESULTS.md` — asserts: recorded as a verified command — `git status` shows no submodule gitlink change and `git diff` contains no `sdk`/`agent-kit`/pgvector pointer move. Recorded honestly as a command plus its output rather than dressed up as a test file that does not exist.
- **TEST-17** (tier: unit) [acceptance] [invariant: INV-7] [covers: ITEM-11] file: `src-app/server/src/modules/llm_repository/utils.rs` — asserts: `capability_url_targets_the_kinds_listing_surface` — the probe URL is derived from the ROW (HF filters by `author=<org>`; Unknown appends to the row's PATH), with a no-path control that keeps the bare-origin probe. **Verified RED first on upstream/main**: `left: Some(".../models/api/models?limit=1")` vs `right: Some(".../api/models?limit=1")`.
- **TEST-18** (tier: unit) [covers: ITEM-11] file: `src-app/server/src/modules/background_mcp/tools.rs` — asserts: `every_spawn_refusal_is_actionable` — every refusal, including the missing-`spec` one, carries a full ARGUMENTS example a model can copy verbatim. Also verified RED first on upstream/main.
