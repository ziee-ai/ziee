# DRIFT-1 — implementation vs plan

Reconciling the Phase-5 implementation against PLAN.md / TESTS.md.

- **DRIFT-1.1** — verdict: impl-wins — TEST-2 lives in the existing
  `src-app/server/tests/chat/stub_chat_tier2_test.rs` (two appended
  `#[tokio::test]`s) instead of a new `empty_completion_test.rs`. Rationale: that
  file already owns the `StubChat` harness (`run_turn`/`create_model`/`chat_perms`)
  the test needs; a new file would duplicate ~70 lines of scaffolding for no gain
  and add a `mod.rs` wiring step. PLAN.md + TESTS.md amended to point at the reused
  file. No behaviour change.

- **DRIFT-1.2** — verdict: impl-wins — the frontend predicate was extracted into
  `src-app/ui/src/modules/chat/components/emptyCompletion.ts`
  (`hasVisibleAnswer` + `isVisibleAnswerBlock`) and unit-tested via
  `emptyCompletion.test.ts` (not the placeholder `hasVisibleAnswer.test.ts` name in
  the plan). Rationale: extracting the predicate keeps `ChatMessage.tsx` thin and
  makes the rule unit-testable without rendering. PLAN.md Files-to-touch + TESTS.md
  TEST-3 amended. No behaviour change.

- **DRIFT-1.3** — verdict: none — all other items implemented exactly as planned:
  `is_visible_answer` + `produced_visible_content` (ITEM-1/2), the `_ =>` terminal
  arm warn + `finish_reason:"empty"` (ITEM-3), the gated inline `Alert`
  (ITEM-4/5, tone=warning per DEC-3), and the regenerated testid registry (ITEM-6).
  `cargo check -p ziee` clean (one pre-existing unrelated dead-code warning); UI
  `tsc --noEmit` clean; the 4 frontend unit tests + the backend unit test compile.

- **DRIFT-1.4** — verdict: none — process update (base branch = `origin/khoi`, PR
  targets `khoi` not `main`) folded into STATUS. khoi == main == 700bf5a1 today, so
  the diff base is unchanged; lifecycle gates run with `--base origin/khoi`.

**Unresolved drifts:** 0
