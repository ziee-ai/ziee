# DRIFT-1 — implementation vs plan

Audited the built diff (`git diff main...HEAD`) against PLAN.md / DECISIONS.md /
TESTS.md after completing all items. tsc clean in BOTH workspaces; all 20 new
unit tests green; lint:colors + lint:guardrails clean.

- **DRIFT-1.1** — verdict: impl-wins — PLAN "Files to touch" originally kept the
  delimited parse inside `DelimitedTable.tsx` and the chunk logic inline in
  `RawCodeView.tsx`. The unit suite runs under `node:test` (no jsdom) and importing
  the `.tsx` viewers pulls React/antd at module eval, so the pure logic was
  extracted into NEW modules `viewers/shared/chunking.ts` +
  `viewers/tabular/parse.ts`. PLAN Files-to-touch amended to list them. Behaviour
  unchanged (the viewers import the same functions/constants). Better structure +
  testability.
- **DRIFT-1.2** — verdict: resolved — DEC-5 originally claimed CSV/TSV is
  "byte-bounded 1:1" and needs NO row cap. That was wrong: the 10 MB byte cap
  bounds bytes, not row COUNT (a 10 MB CSV of `a\n` rows is millions of records).
  Corrected to a raised `DELIMITED_MAX_ROWS = 300_000` OOM backstop (parallel to
  text + xlsx). DECISIONS (DEC-5/DEC-10) + TESTS (TEST-4) updated before
  implementation; the implementation matches the corrected decision. Strictly
  safer; does not change the acked client-side / no-server-paging approach. The
  user was told about this correction in the phase-5 kickoff message.
- **DRIFT-1.3** — verdict: impl-wins — the large-table e2e needs a real xlsx
  binary upload, so a `seedProjectBinaryFile` helper was added to
  `tests/e2e/file/helpers.ts` (mirrors the existing `seedProjectImage` binary
  path). Test infra only; PLAN Files-to-touch amended.
- **DRIFT-1.4** — verdict: impl-wins — the state-matrix detector flags
  RawCodeView's IntersectionObserver effect guard (`chunks.length === 0`) as an
  "empty" render state. It is unreachable (`chunkLineArray` always yields ≥1
  chunk), so a `skip` coverage entry with that accurate rationale was added to
  `src/dev/gallery/stateCoverage.ts`. PLAN Files-to-touch amended. The viewer's
  real render is genuinely covered by the new `seeded-rawcode-large` gallery
  surface + the large-text e2e (not a coverage dodge).
- **DRIFT-1.5** — verdict: none — mechanically-regenerated manifests
  (`galleryCoverage.generated.ts`, `stateMatrix.generated.ts`, `STATE_MATRIX.md`,
  `testIds.generated.ts`, `fixtures/crawl.generated.ts`) changed as expected from
  the new seeded surfaces + testids; produced by the committed gen scripts, so
  they are derived-not-authored. No divergence.
- **DRIFT-1.6** — verdict: none — ITEM-3 planned to touch
  `useFindInDocument.ts` "only if hardening is needed". It was NOT needed: keeping
  every line's text node in the DOM (plain or highlighted) means the TreeWalker
  already spans the whole file, and the plain→highlight swap preserves textContent
  (unit TEST-6). find is left byte-for-byte unchanged, matching ITEM-3's intent.

**Unresolved drifts:** 0
