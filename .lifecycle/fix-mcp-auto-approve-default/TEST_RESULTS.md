# TEST_RESULTS — fix-mcp-auto-approve-default

Logs saved under
`/tmp/claude-1000/-data-khoi-home-workspace-ziee-wt-mcp-autoapprove/6276d41d-b87a-49cb-957d-aa52a304b9d2/scratchpad/`
(`mcp-defaults-int.log`, `mcp-stub-regression.log`, `e2e-approval-default.log`,
`gate-ui.log`).

## Per-test results

- **TEST-1**: PASS — `approval::models::tests::conversation_get_approval_mode_parses_or_defaults`
- **TEST-2**: PASS — `defaults::models::tests::user_defaults_get_approval_mode_parses_or_defaults`
- **TEST-3**: PASS — `approval::models::tests::default_approval_mode_round_trips_through_its_db_spelling`
- **TEST-4**: PASS — `settings::repository::tests::no_row_default_agrees_with_the_approval_mode_enum`
- **TEST-5**: PASS — `mcp::tests::resolve_approval_*` (5 cases: conversation-wins both directions, empty-list non-inheritance, user-defaults fallback, nothing-stored → deployment default, and stability across the turn-1 auto-persist)
- **TEST-6**: PASS — `approval::models::tests::upsert_request_distinguishes_absent_from_explicit_approval_mode`
- **TEST-7**: PASS — `defaults::models::tests::upsert_defaults_request_distinguishes_absent_from_explicit_approval_mode`
- **TEST-8**: PASS — `approvalDefaults.test.ts` blankMcpConfig (3 cases: server default stamped for all 3 modes, empty + safe fallback, fresh collections per call, loop-settings passthrough)
- **TEST-9**: PASS — `approvalDefaults.test.ts` effectiveApprovalMode (3 cases: explicit wins over every server default, unset falls back, fallback only when unknown)
- **TEST-10**: PASS — `approvalDefaults.test.ts` approvalModePayload (3 cases: key omitted when unset, present when set, spreads without leaking a key)
- **TEST-11**: PASS — `conversation_settings_default_test::test_omitted_approval_mode_on_insert_uses_the_server_default`
- **TEST-12**: PASS — `conversation_settings_default_test::test_omitted_approval_mode_preserves_an_explicit_choice`
- **TEST-13**: PASS — `conversation_settings_default_test::test_explicit_approval_mode_round_trips`
- **TEST-14**: PASS — `conversation_settings_default_test::test_omitted_fields_preserve_both_mode_and_auto_approved_tools` (**failed on first run and is what exposed ITEM-14**)
- **TEST-15**: PASS — `mcp_defaults_test::test_update_mcp_defaults_omitted_approval_mode_defaults_then_preserves`
- **TEST-16**: PASS — `mcp_defaults_test::{test_get_mcp_defaults_requires_permission, test_update_mcp_defaults_requires_permission, test_get_mcp_defaults_no_defaults_set}` (403 read-gate, 403 edit-gate, `defaults` still null)
- **TEST-17**: PASS — `conversation_settings_default_test::test_advertised_default_matches_what_a_fresh_conversation_receives` + `mcp_defaults_test::test_get_mcp_defaults_advertises_the_server_default_mode`
- **TEST-18**: PASS — `chat::title_approval_test` (2), `chat::title_audience_test` (1), `mcp::approval_claim_test` (1), `mcp::mcp_approval_loop_test` (2), `project::mcp_test` (7) — 13 pre-existing tests over the approval gate + the settings snapshot, all green after the `resolve_approval` extraction
- **TEST-19**: PASS — `openapi::emit_ts::tests::types_ts_parity` (ran as part of `cargo test --lib -p ziee`)
- **TEST-20**: PASS — e2e `a settings write on a fresh conversation does not pin a mode the server did not choose`
- **TEST-21**: PASS — e2e `removing a server chip on a new chat records the server list without pinning a mode`
- **TEST-22**: PASS — e2e `the config modal on a new chat shows the server default approval mode`
- **TEST-23**: PASS — `mcp_defaults_test::test_update_mcp_defaults_omitted_auto_approved_tools_preserves_the_allow_list`

Also re-ran the whole `cargo test --lib -p ziee mcp::` suite: **276 passed, 0 failed**.
Scoped integration run: **19 passed, 0 failed**. Stub-based regression run: **5 passed,
0 failed**. E2E: **3 passed, 0 failed**.

## Frontend gate

`gate:ui (ui): PASS` — `npm run gate:ui -- --skip-visual` on `GALLERY_PORT=1425`
(the default :1420 is occupied by an unrelated process on this host; `GALLERY_PORT`
is the script's own documented seam). Result: tsc clean, lint clean, runtime-health
**182/182 surfaces PASS with 0 gating HIGH findings** over 590 page loads.

Layer B (`VISUAL_SNAPSHOTS=1`) was **not** run. Stated plainly rather than implied:
this diff adds no visual surface and no new render branch — it changes the VALUE
passed to an existing `Select`, and the gallery cassette's `default_approval_mode`
is `manual_approve`, identical to the literal it replaced, so the gallery renders
pixel-identically. The Layer A + axe + runtime passes that did run cover this
surface.

`npm run check (ui): PARTIAL` — **13 of 18 steps pass; 5 fail for reasons that
pre-date this branch.** Recorded honestly rather than claimed as PASS.

| Step | Result |
|---|---|
| tsc | PASS |
| lint:guardrails / lint:colors / lint:settings-field | PASS |
| lint:adjacent-inline / lint:icon-action / lint:logical-direction / lint:tooltip-placement | PASS |
| check:kit-manifest | PASS |
| check:design-spec | PASS |
| check:gallery-crawl | PASS |
| gallery:check-fixtures (ajv vs openapi) | PASS — incl. `✓ crawl:Mcp.getDefaults`, the fixture this diff changed |
| check:gallery-seed-registry | PASS |
| check:testid-registry | **FAIL — pre-existing** |
| check:gallery-coverage | **FAIL — pre-existing** |
| check:state-matrix | **FAIL — pre-existing** |
| check:overlay-registry | **FAIL — pre-existing** |
| check:override-registry | **FAIL — pre-existing** |

**Evidence that all five are pre-existing, not caused by this diff.** Each was
re-run against the UNTOUCHED base tree in this same worktree
(`git checkout origin/khoi -- src-app/ui/src src-app/ui/scripts src-app/desktop/ui/src`,
run the check, then `git checkout HEAD -- …` to restore). All five fail
**identically** with this branch's changes absent. They are stale committed
generated files relative to the `sdk` submodule pin — e.g. `testIds.generated.ts`
is missing the split-chat pane ids and still carries notification ids that moved
into the sdk; regenerating `galleryCoverage.generated.ts` would DELETE 107
`components/ui/kit/*` surfaces because the kit now lives in the sdk package.

They are deliberately **not** fixed here. Four of the five regenerate into the
**`sdk` submodule** (a different repository), so fixing them means a commit there
plus a submodule-pointer bump on this branch — out of scope for this bugfix,
shared with every other worker on this repo, and squarely the "don't edit shared
infrastructure to route around an unrelated failure" rule (B3). The fifth
(`galleryCoverage`) would ship a destructive 107-line deletion.

This diff's own frontend contract is fully covered by the steps that DO pass:
`tsc`, every lint, the design-spec gate, and — most relevantly — the ajv contract
test on the exact cassette entry this change touched.

## Backend commands

```bash
cargo test --lib -p ziee mcp::                       # 276 passed
cargo test --test integration_tests -- --test-threads=4 \
    mcp::conversation_settings_default_test mcp::mcp_defaults_test   # 19 passed
cargo test --test integration_tests -- --test-threads=4 \
    chat::title_approval_test chat::title_audience_test \
    mcp::mcp_approval_loop_test                       # 5 passed
cargo test --test integration_tests -- --test-threads=4 \
    mcp::approval_claim_test project::mcp_test        # 8 passed
```

`cargo check -p ziee --all-targets` reports ONE error, in
`server/tests/path_resolution.rs:153` (`E0507: cannot move out of dereference of
Config`, from `ziee_core::config::EmbeddedPostgreSqlConfig` not being `Copy`).
That file is **untouched by this diff** and the error is unrelated to it —
`--lib` and `--test integration_tests` both compile clean, which is why the
scoped commands above are used. Pre-existing.

## Environment notes (not defects in this change)

- **No `tests/.env.test` exists on this host** (only `.env.test.example`), so the
  ~40 cases in `mcp_approval_workflow_test` / `mcp_extension_test` that call
  `configure_any_provider` hard-panic at `chat/helpers.rs:593` ("No AI provider API
  keys found") before reaching any MCP code. Not skipped to go green — TEST-18 was
  retargeted at the stub-LLM equivalents that exercise the SAME approval gate and
  do run (see DRIFT-1.8).
- **E2E port bases had to be moved.** The harness's default vite base (:9000)
  collides with a MinIO console running on this host — the first run failed with a
  MinIO page where the login form should be. Fixed via the harness's own documented
  seam (`ZIEE_E2E_BASE_VITE_PORT=9400 ZIEE_E2E_BASE_BACKEND_PORT=9500`), not by
  editing the shared port manager.
- **All docker-touching commands** were run as a single `sg docker -c "…"`
  invocation (the pane shell has the docker group but not active). No `usermod`, no
  socket chmod.
