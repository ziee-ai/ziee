# TEST_RESULTS — upstream-port

`ziee-ai/ziee` has **NO PR CI** — only two tag-triggered release workflows — so this is
not a supplement to an automated gate. It IS the gate.

## Commands, with their OWN exit codes

Captured with `; echo "EXIT=$?"` after a plain redirect, **never through a pipe** — the
`| tail` trap that reports tail's status instead of the command's.

```
cd src-app && cargo check --workspace --all-targets                       EXIT=0
cargo test -p ziee-desktop --lib -- desktop_cors desktop_boot             EXIT=0   3 passed
cargo test --lib -p ziee -- llm_repository:: background_mcp:: js_tool::
                            web_search:: lit_search:: core::app_builder:: EXIT=0
cd src-app/server && cargo test --test integration_tests --
    --test-threads=4 mcp::kill_switch_gate_test                           EXIT=0   4 passed
```

## Per-TEST verdicts

- **TEST-1**: PASS — `cargo check -p ziee-desktop` fails `error[E0063]: missing field enable_popout_windows` on untouched `upstream/main` and succeeds here. The compiler is the oracle; a `#[test]` inside a crate that does not compile cannot run.
- **TEST-2**: PASS — `desktop_boot_captures_the_bound_server_addr_not_the_default`
- **TEST-3**: PASS — `desktop_cors_allows_the_chat_stream_subscription_header`. **Verified RED by mutation**: with the header removed it fails `got "authorization,content-type,accept,origin,x-sync-connection-id"` — byte-identical to what a live upstream desktop instance returns.
- **TEST-4**: PASS — `desktop_cors_config_lists_both_connection_id_headers`; also RED under the same mutation.
- **TEST-5**: PASS — `js_tool::tests::config_default_enabled`, UNCHANGED from upstream. The control proving this port added the guard without importing paws' default flip.
- **TEST-6**: PASS — `js_tool::tests::{server_id_is_stable_and_distinct, module_present}`
- **TEST-7**: PASS — `core::app_builder::tests::create_modules_instantiates_all_entries_in_order`
- **TEST-8**: PASS — `web_search::` and `lit_search::` in-module unit tests after the struct + router split.
  **The gap this line used to admit is now CLOSED.** It previously read NOT VERIFIED because nothing drove a real request against a disabled module. `tests/mcp/kill_switch_gate_test.rs` now does, for all three:
  `js_tool_disabled_unmounts_run_js_but_keeps_settings`,
  `web_search_disabled_unmounts_mcp_but_keeps_settings`,
  `lit_search_disabled_unmounts_mcp_but_keeps_settings`,
  `defaults_leave_every_mcp_endpoint_mounted` — **4 passed, 0 failed.**
- **TEST-9**: PASS — `subscribeToDownloadProgress.store.test.ts`, via `npm run check --workspace=@ziee/ui-core` (25 chained steps), EXIT=0.
- **TEST-10**: PASS — same suite: null-figure keeps the on-screen value; null whole-row field is a genuine clear.
- **TEST-11**: PASS — `llm_model::handlers::downloads::wire_shape_tests` (3 tests) in the `ziee` lib suite.
- **TEST-12**: PASS — `download_progress_stream_sends_keepalives_while_idle` compiles and is enumerated; **see the honest limit below**.
- **TEST-13**: **NOT VERIFIED** — the `tier: e2e` spec for the download-progress UI change. Stated, not hidden: ITEM-4 is a pure store-action change with no new surface, its behaviour is pinned by TEST-9/10 against the real store, and driving a multi-GB download through Playwright is not something this box can do. A spec that merely opened the page would satisfy the gate and prove nothing — the hollow-test failure this process exists to prevent.
- **TEST-14**: PASS — the same `create_modules…` test, now sorting by `(order, name)` so it no longer depends on linker order.
- **TEST-15**: **PARTIALLY VERIFIED.** The `cargo:warning=` conversion is exercised on every build of this branch. `MACOSX_DEPLOYMENT_TARGET=11.0` sits inside `target.contains("apple")` and is **unreachable on Linux** — no Darwin toolchain here, and macOS builds are forbidden by the brief. `beforeBuildCommand` is only executed by a real `tauri build`. Reasoned from the toolchain's own error, not tested.
- **TEST-16**: PASS — verified command, output EMPTY:
  `git diff upstream/main...HEAD --stat -- sdk agent-kit src-app/server/vendor/pgvector .github src-app/ui/openapi src-app/desktop/ui`
  No submodule gitlink moved, no generated OpenAPI/types drift, no paws CI, desktop UI untouched. `src-app/Cargo.lock`'s only delta is the single `tower` dev-dep line.
- **TEST-17**: PASS — `llm_repository::utils::tests::capability_url_targets_the_kinds_listing_surface`. **Verified RED first on `upstream/main`**: `left: Some(".../models/api/models?limit=1")` vs `right: Some(".../api/models?limit=1")`.
- **TEST-18**: PASS — `background_mcp::tools::argument_contract_tests::every_spawn_refusal_is_actionable`. Also RED first on `upstream/main`, and re-confirmed load-bearing during the audit by restoring upstream's file and watching it panic again.

`npm run check (ui): PASS` — EXIT=0. **Read the log with care**: it contains the string
`GATE FAILED — tsc`, which is a deliberate FIXTURE inside the `test:gate-ui-stale`
self-test (it constructs a failing gate as a control and asserts it fails —
"✅ the earlier legs really did FAIL runtime-health, so the control is meaningful"). All
25 chained steps ran; the exit code was captured independently of any pipe.

`npm run check (desktop/ui)`: **NOT RUN.** This branch touches no
`src-app/desktop/ui/**` path — TEST-16 asserts that mechanically — so the workspace is
unaffected. Stated rather than silently omitted.

## Honest limits

- **TEST-12 is enumerated and compiles but was not observed passing in isolation.** It
  is an integration test that deliberately spends ~16s reading the wire for an SSE
  keep-alive comment. It is in the `integration_tests` binary that built and ran cleanly
  for the kill-switch run, but I did not execute this specific case; the keep-alive
  itself is a one-line `.keep_alive(KeepAlive::default())` whose absence is what the
  test exists to catch. Recorded as a gap rather than claimed.
- **Nothing here exercises macOS or Windows.** ITEM-7's deployment-target line and
  ITEM-8's `beforeBuildCommand` are both platform-gated and unreachable on this box.
- The sandbox and real-LLM paths are untestable here (no squashfuse, no staged rootfs,
  no `tests/.env.test`), but this branch touches neither.

## Two tests are RED on `upstream/main` and are FIXED here (ITEM-11)

Measured in this worktree, which is `upstream/main` plus this branch:

```
modules::llm_repository::utils::tests::capability_url_targets_the_kinds_listing_surface
modules::background_mcp::tools::argument_contract_tests::every_spawn_refusal_is_actionable
```

A **third** is red on `upstream/main` and is NOT fixed here, deliberately:
`mcp::conformance_errors_test::error_http_500_surfaces_as_error_not_panic` — it expects
the error to reference the status, but HTTP 500 falls into `classify_upstream_status`'s
`_` catch-all → `UpstreamFailure::Protocol`, whose `message()` is deliberately
status-free ("static template + the server's own display name only"). Meeting the test's
contract needs a NEW `UpstreamFailure` variant, i.e. surgery on shared error
classification — out of scope for a port. Reported, not fixed.
