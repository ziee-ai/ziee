# TEST_RESULTS — js-tool-scripting

Phase 8 complete. All enumerated tests (unit + integration + e2e) are green,
including the real-LLM smoke (TEST-36), which ran GREEN against the live Qwen
bridge once the engine came up (DRIFT-2, resolved). Backend diff → the cargo tiers
apply; ui diff → the frontend gate + e2e apply.

## Frontend static gate (ui workspace)

- npm run check (ui): PASS

## UI evaluator gate (gallery runtime-health — `npm run gate:ui`)

- tsc: PASS · lint (guardrails + colors): PASS
- runtime-health for the run_js surfaces: PASS — **zero** HIGH findings reference
  `run_js` / `JsToolApprovalContent` / `run_js_approval` (the approval prompt is
  via-rendered inline in a chat message; the run_js call-card reuses the existing
  `McpToolUseRenderer`, introducing no new gallery state — hence the `via`
  coverage allowlist entry, accepted by `check:gallery-coverage`).
- The overall `gate:ui` command is red on **5 pre-existing baseline surfaces that
  this branch provably does not touch** (`git diff origin/main...HEAD` on
  `src-app/ui/src/**` touches only the mcp chat-extension + the new
  JsToolApprovalContent + the tool-calls source column + generated gallery files —
  none of the modules below): `seeded-llm-models-loading` ("Rendered more hooks
  than during the previous render" crash in the llm-model surface),
  `deep-chat-right-panel-file` (transparent-fg icon-span contrast),
  `deep-chat-rendering-showcase` (KaTeX web-font load / harness 403s),
  `deep-chat-long` (`File.store.ts` loadFileTextContent), `seeded-s3-group-widget-error`.
  These are outside this feature's scope and DoD (the DoD is per-surface; the
  run_js surfaces have zero HIGH).

## Unit tier (lib `#[cfg(test)]`, all green)

- **TEST-1**: PASS
- **TEST-2**: PASS
- **TEST-3**: PASS
- **TEST-4**: PASS
- **TEST-5**: PASS
- **TEST-6**: PASS
- **TEST-7**: PASS
- **TEST-8**: PASS
- **TEST-10**: PASS
- **TEST-13**: PASS
- **TEST-14**: PASS
- **TEST-16**: PASS
- **TEST-17**: PASS
- **TEST-20**: PASS
- **TEST-21**: PASS
- **TEST-23**: PASS
- **TEST-26**: PASS
- **TEST-29**: PASS
- **TEST-34**: PASS (openapi `types_ts_parity` + `types_ts_parity_desktop`, both green)
- **TEST-37**: PASS

## Integration tier (`tests/js_tool/mod.rs`, green — `--test-threads=1`)

- **TEST-9**: PASS
- **TEST-15**: PASS
- **TEST-18**: PASS
- **TEST-19**: PASS
- **TEST-25**: PASS
- **TEST-27**: PASS
- **TEST-28**: PASS

## E2E tier (`src-app/ui/tests/e2e/chat/`, Playwright `--workers=1`, all green)

- **TEST-11**: PASS (run-js-inner-approval.spec.ts — approve resolves via side-channel `/respond` with `accept`; status → approved)
- **TEST-12**: PASS (run-js-inner-approval.spec.ts — deny resolves via `/respond` with `decline`; status → denied)
- **TEST-30**: PASS (run-js-inner-approval.spec.ts — the resolve issues `POST /api/mcp/elicitation/{id}/respond`, observed via network mock)
- **TEST-31**: PASS (run-js-inner-approval.spec.ts — the full inner-approval prompt→resolve flow through the real `JsToolApprovalContent`)
- **TEST-32**: PASS (run-js-tool-scripting.spec.ts — the McpToolCallsTab renders the `script` source tag with its own tone, not the fallback)
- **TEST-33**: PASS (gallery runtime-health via `npm run gate:ui` — the run_js surfaces contribute zero HIGH; see the UI evaluator gate note above)
- **TEST-35**: PASS (run-js-tool-scripting.spec.ts — a mocked run_js call renders exactly ONE run_js tool card carrying the final summary, not N intermediate cards)
- **TEST-36**: PASS (run-js-real-llm.spec.ts — ran GREEN against the live Qwen bridge, `qwen3.6-35b-a3b`, 1 passed / no retries: the real model chose run_js, the embedded runtime executed it end-to-end (`ToolUse(run_js)`→`ToolResult(run_js)`), the card reached `completed`, and the answer reflected 6*7=42 — provider-independence proven end-to-end. Self-skips without `OPENAI_BASE_URL`/`ZIEE_TEST_LLM_MODEL`. See DRIFT-2 (resolved).)

## Admin-configurable limits increment (TEST-38..53)

### Unit (lib `#[cfg(test)]`, all green — 32/32 in the js_tool/openapi run)
- **TEST-38**: PASS (settings::{empty_patch_validates, memory_bytes_bounds, max_stack_bytes_bounds, secs_and_counts_bounds} — validate() bounds)
- **TEST-39**: PASS (limits::from_settings_maps_tunables_and_keeps_defaults)
- **TEST-40**: PASS (settings_cache::defaults_match_migration)
- **TEST-41**: PASS (executor::test_apply_resize_grows_and_shrinks — global-sem resize math)
- **TEST-49**: PASS (openapi::emit_ts::types_ts_parity + types_ts_parity_desktop, green after regen with the new JsToolSettings schemas/perms/entity)

### Integration (`tests/js_tool/settings.rs`, 14/14 green incl. regression — `--test-threads=1`)
- **TEST-42**: PASS (GET returns the seeded migration defaults)
- **TEST-43**: PASS (PUT updates + persists; PATCH preserves untouched fields)
- **TEST-44**: PASS (PUT rejects 8 out-of-range values with 422; row unmutated)
- **TEST-45**: PASS (GET+PUT 403 for a user lacking js_tool::settings::{read,manage})
- **TEST-46**: PASS (GET+PUT 401 unauthenticated)
- **TEST-47**: PASS (db-value-honored-at-execution — the 40 MiB alloc succeeds at the 128 MiB default and OOMs after PUTing memory_bytes=16 MiB, proving the DB value + cache invalidation flip the live run)
- **TEST-48**: PASS (PUT emits SyncEntity::JsToolSettings/update to the read audience; a user without read stays silent, via SyncProbe)

### E2E (`tests/e2e/settings/js-tool-settings.spec.ts`, Playwright `--workers=1`)
Each spec PASSED (no assertion failure ever observed). The backend-start
failures seen in some runs are the known environmental combo — the stale
`macros` proc-macro warmup-build gotcha ([[project_macros_stale_chat_extensions]])
+ shared-host concurrent-e2e load exceeding the hardcoded 120 s server-start
budget ([[project_e2e_test_contention]]) — never a test-logic failure.
- **TEST-50**: PASS (admin edits wall_secs + Save; value persists across a reload — GET/PUT round-trip; waits on the PUT response before reload)
- **TEST-51**: PASS (out-of-range value clamps to the bound on blur; a direct absurd PUT returns 422 — validation-rejects-absurd, visible)
- **TEST-52**: PASS (a REST change to max_concurrent_runs on device A reflects live on device B via sync:js_tool_settings, no reload)
- **TEST-53**: PASS (a non-admin lacking js_tool::settings::read hits the settings-forbidden guard; the card never renders)
