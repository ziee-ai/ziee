# FIX_ROUND-3 — js-tool-scripting (phase-8 delta re-audit)

Phase 8 added new committed hunks (the integration test file, TEST-21/TEST-23
unit tests, stub-chat plans, the rquickjs Cargo.lock tree) plus the e2e specs +
the `$.elicitationRequests` handler fix — all POSTDATING the phase-6/7 blind
audit. The coverage law flagged them uncovered, so a fresh 4-angle blind round
was run over the entire phase-8 delta (no reasoning handed to the reviewers):

- **tests-quality** — the test assertions actually prove their claims (non-cosmetic).
- **correctness** — selectors/ids/endpoints/enums/values match the code under test.
- **security-supplychain-and-patterns** — the Cargo.lock additions + test-file conventions.
- **error-handling / dependency-correctness / patterns-conformance** (gap-fill) — the
  unit-test hunks + lock feature-resolution.

## Confirmed findings (all FIXED)

- **F1 (tests-quality, medium)** — `run-js-real-llm.spec.ts`: asserted only that a
  run_js card mounted, not that the script executed/returned; an always-erroring
  run_js would pass. FIXED: also assert a toolcall reaches `data-status=completed`
  and the model's answer contains the computed value `42`.
- **F2 (tests-quality, medium)** — `js_tool/mod.rs` TEST-15: asserted 3 recorded
  sub-calls + a summary reached the model, but NOT that the intermediate results are
  ABSENT from context (the core PTC-economics claim — a naive N-tool-result loop would
  pass the row-count too). FIXED: added `context_tool_use_names()` and assert the
  run_js call IS a context `tool_use` block while the 3 in-script `get_tool_result`
  sub-calls are NOT.
- **F3 (tests-quality, low)** — `run-js-tool-scripting.spec.ts`: asserted the source
  tag TEXT='script' but not its distinct tone. FIXED: also assert `toHaveClass(/text-info/)`
  (SOURCE_TONE.script='info', proving the non-fallback tone).
- **F4 (correctness, low)** — `js_tool/mod.rs`:230: comment said `return 6*42` (=252)
  but the stub runs `return 6*7` (=42). FIXED: comment corrected.

The Cargo.lock supply-chain review and the unit-test/stub/mod hunks came back CLEAN
(0 findings — exactly the rquickjs 0.12.1 tree, features match Cargo.toml; the unit
tests assert the real seams).

**New confirmed findings:** 4
