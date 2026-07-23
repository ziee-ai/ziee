# HUMAN_FEEDBACK — workflow-kind-agent (Phase 9)

Human critiques + steering received during this feature, each resolved.

- **FB-1** [status: resolved] — the product owner picked, via option-picker, "General visual workflow
  builder" (author all 6 step kinds; agent gets the deep-friendly treatment) + "Friendly
  domain-language timeline" for the run UX → the feature was scoped + built exactly to that split
  (builder + friendly agent form + activity timeline). [generalizable: yes — surface a genuine
  product fork as an AskUserQuestion picker BEFORE finalizing the plan, don't guess scope.]

- **FB-2** [status: resolved] — "You announced Phase 3 ~40min ago but there's no TESTS.md and no new
  commit — looks like you idled after the announcement." → I had announced a phase then stalled; fixed
  by immediately writing + gating + committing TESTS.md and every subsequent phase without an
  announce-then-idle gap. [generalizable: yes — never announce a phase and then idle; execute the
  work in the same turn and only report at a real boundary.]

- **FB-3** [status: resolved] — "before Phase 8, RECONCILE the 3 HIGH findings that still read
  'confirmed' in LEDGER.jsonl … verify each is genuinely FIXED … update its ledger status to 'fixed'
  with the fix-commit + file:line evidence." → verified all 3 HIGHs against the current tree
  (validate-before-wipe at dev.rs:549; llm/llm_map tools picker removed; delete→create save), flipped
  their ledger status to `fixed` with commit + file:line evidence, and reconciled the remaining 19
  phase-6 rows (all fixed in the converged loop). [generalizable: yes — after a fix loop, reconcile
  the ledger statuses to `fixed` with evidence; a stale `confirmed` on a shipped fix is misleading.]

- **FB-4** [status: resolved] — chose Option 1 for the sdk kit-testid registry: "LIFECYCLE_CLEAN_TREE_IGNORE
  for the generated testIds file … the sdk POINTER is unchanged … committing to the sdk + bumping the
  pointer is a CROSS-REPO action the human coordinates at merge … BUT document it." → regenerated the
  registry on-disk (so `check:testid-registry` passes), added the `sdk` clean-tree ignore to
  `.claude/app.config` with an explanatory comment, and documented the carry-along in `CROSS_REPO.md`.
  Did NOT autonomously commit to the sdk / bump the pointer. [generalizable: yes — a generated file in
  a submodule whose pointer is unchanged is a merge-time cross-repo carry-along: keep it regenerated +
  ignore in clean-tree + document, don't autonomously commit cross-repo.]

- **FB-5** [status: resolved] — chose Option 2 for the gallery-coverage/state-matrix base debt: "record
  as pre-existing debt, caveated PASS … it FAILS ON THE BASE too … the fix touches 73 entries inside
  live1's active SDK-extraction domain … Do NOT do the app-local cleanup, do NOT pause … BUT
  scope-check YOUR part: confirm your 7 workflow surfaces ARE correctly present." → confirmed via a
  throwaway regen that all 16 builder/run components + the timeline are covered (my feature's coverage
  is sound), reverted the regen so the 73 defunct-kit base entries are untouched, and documented the
  4 failing gallery-registry checks as pre-existing SDK-extraction base debt in TEST_RESULTS.md +
  CROSS_REPO.md — not fixed on this branch. [generalizable: yes — when a whole-app gate fails on
  debt that also fails on the base AND lives in another active workstream's domain, scope-verify only
  YOUR part is green, document the base debt, and don't fix another workstream's debt to force green.]

No `open` items.
