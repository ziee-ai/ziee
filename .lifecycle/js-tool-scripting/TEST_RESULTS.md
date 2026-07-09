# TEST_RESULTS — js-tool-scripting

Phase 8 complete. All enumerated tests (unit + integration + e2e) are green.
Backend diff → the cargo tiers apply; ui diff → the frontend gate + e2e apply.
The one real-LLM smoke was descoped from the gate (external engine offline) — see
DRIFT-2; its capability is covered by TEST-15 + TEST-20 + TEST-35.

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

## Descoped (not gated) — real-LLM smoke

- `run-js-real-llm.spec.ts` retained as an opt-in smoke (self-skips unless
  `OPENAI_BASE_URL` + `ZIEE_TEST_LLM_MODEL` are set). Wired + attempted against the
  local LLM bridge; the shared vLLM engine behind it (`127.0.0.1:8000`) is OFFLINE
  in this environment and may not be restarted (shared GPU box). See DRIFT-2. The
  capability is covered by TEST-20 (provider-agnostic attach) + TEST-15 (model-emitted
  run_js executes end-to-end through the real dispatcher + real rquickjs) + TEST-35.
