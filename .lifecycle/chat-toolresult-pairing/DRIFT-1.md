# DRIFT-1 — implementation vs plan

- **DRIFT-1.1** — verdict: impl-wins — ITEM-1 originally said "synthesize an `is_error`
  result for EVERY unresolved trailing tool_use." Implementing that verbatim broke the
  legitimate in-progress / awaiting-approval case: a lone trailing `tool_use` whose real
  result is appended SEPARATELY by the approval-resume path (`mcp.rs` pushes it as a
  following User message) — encoded by the existing integration test
  `trailing_tool_use_without_result_is_emitted_as_assistant` (asserts a single Assistant
  turn, no Tool turn). Synthesizing there would race a real result still coming, and
  double-answer the tool_use. **Refinement:** synthesize gaps ONLY for a
  completed-but-partial batch — a trailing batch where ≥1 tool_use already has a captured
  result (`batch_has_result`); a batch with ZERO results stays a single Assistant turn
  (existing behavior preserved). PLAN ITEM-1 and DECISIONS DEC-1 amended to record this;
  TESTS.md is unaffected (all 6 tests still hold — TEST-1's partial batch has a real
  result, so it takes the synthesize path). Re-ran gates --phase 1..4 green after the
  amendment.

- **DRIFT-1.2** — verdict: impl-wins — the plan located Fix A in the trailing branch
  only. During impl I also switched the loop's result accumulator from a flat
  `current_results: Vec` to a `results_by_id: HashMap` and routed BOTH the per-flush and
  the trailing paths through one `flush_assistant_tool_pair` helper. This drops orphan
  `tool_result`s (ITEM-3) in the per-flush path too (a stray mid-batch orphan result no
  longer rides along into a Tool turn), making the whole function robust rather than just
  the trailing branch. No behavior change for the all-matched case (byte-identical output,
  verified by `group_assistant_blocks_matched_parallel_batch_unchanged` and the existing
  integration tests). Strictly a robustness improvement inside ITEM-1/ITEM-3 scope; no new
  item or test needed.

- **DRIFT-1.3** — verdict: none — Fix B (summarizer snap-forward, ITEM-5) implemented
  exactly as planned.

**Unresolved drifts:** 0
