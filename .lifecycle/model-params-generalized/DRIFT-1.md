# DRIFT-1 — implementation vs plan

- **DRIFT-1.1** — verdict: impl-wins — Anthropic & Gemini `build_request_body`
  keep their original signatures and compute `ResolvedParams` internally; only
  OpenAI's signature changed to take `&ResolvedParams` (its caller already needed
  `rp.disable_stream` for the non-streaming dispatch). Same wire outcome, less
  churn. PLAN's generic "build_request_body(req, &rp)" amended in spirit.
- **DRIFT-1.2** — verdict: impl-wins — ITEM-12 no longer auto-bakes inferred
  capability defaults at create time. The three fields round-trip through the
  EXISTING `capabilities` JSONB create/update path with zero handler changes;
  silent rows resolve dynamically at request time (avoids freezing inference).
  PLAN.md ITEM-12 amended.
- **DRIFT-1.3** — verdict: impl-wins — `discoveredModelForm.ts` is unchanged: the
  new tri-state toggles default to Auto (undefined), with no discovery-time
  inference to map. TEST-20 (discoveredModelForm maps inferred defaults) is
  therefore removed; ITEM-14 is covered by TEST-21 (e2e) + the `npm run check`
  tsc gate on the new controls. TESTS.md amended.
- **DRIFT-1.4** — verdict: resolved — the e2e spec lives at
  `tests/e2e/llm/model-capability-toggles.spec.ts` (the repo's actual llm e2e
  dir), not the enumerated `05-llm/`. TESTS.md path corrected.
- **DRIFT-1.5** — verdict: impl-wins — ITEM-11 needed no new server control-flow
  code: the chat loop keys off `finish_reason.is_some()` (not string matching),
  and the mcp/sampling handler's default arm passes canonical values through. One
  advisory nuance: Anthropic `stop_sequence` now canonicalizes to `stop`, so the
  MCP sampling `stopReason` reports `endTurn` instead of `stopSequence` — an
  acceptable fidelity loss in an advisory field.
- **DRIFT-1.6** — verdict: none — `family_thinking_style` was refined to exclude
  Haiku (a bare `claude major>=4` would wrongly enable thinking for haiku-4-5);
  within ITEM-1's intent, caught before testing.
- **DRIFT-1.7** — verdict: none — `from_anthropic_error` re-gained body
  sanitization (defense-in-depth) since the Anthropic status-error path now routes
  through it; independent of the reverted PR #122, a safe hardening.

**Unresolved drifts:** 0
