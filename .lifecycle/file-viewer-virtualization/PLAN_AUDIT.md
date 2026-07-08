# PLAN_AUDIT — file-viewer-virtualization

Audited the plan against the codebase (worktree off origin/main). This is a
frontend-only change to the file-viewer surfaces.

## Breakage risk

- **RawCodeView (ITEM-1..4)** is the highest-risk edit — it is consumed by the
  text viewer (`text/body.tsx`), the markdown RAW mode (`markdown/body.tsx`), and
  the web viewer's raw branch (`web/body.tsx`). All three pass `text`,
  `filename?`, `wordWrap?` and rely on: (a) the `data-testid="raw-code-view"`
  root + `data-word-wrap` attribute (e2e `word-wrap.spec.ts`), (b) all line text
  present in the DOM for find (`find-in-document.spec.ts`). The plan explicitly
  preserves both (keep the root testid + attribute; keep all lines in the DOM).
  Risk contained by ITEM-3/ITEM-4 preservation clauses + the e2e regressions in
  TESTS.md. Inline-preview callers (no `fileId`) render `RawCodeView` directly
  without `FindableRegion` — the chunk-windowing must not depend on a
  FindableRegion wrapper (it does not; it is self-contained in RawCodeView).
- **DelimitedTable / XlsxBody (ITEM-5/6)** — removing the `slice(0, MAX_ROWS)`
  head-cap grows `dataSource`. Callers already flip to the kit `Table`'s
  `virtualized` path above `VIRTUALIZE_ROW_THRESHOLD=200`, which mounts only
  visible rows, so DOM cost does not grow with row count. The only growth is the
  in-memory `dataSource` array + the per-cell record objects — bounded by the
  10 MB byte cap for CSV/TSV (1:1) and by the raised `XLSX_MAX_ROWS` backstop for
  xlsx (see Migration/OOM note below). `onViewChange`, export, copy, jump all
  operate over `viewRef`/`dataSource` and are unaffected in shape.
- **XlsxBody parse (ITEM-6)** — `XLSX.read({ sheetRows: MAX_ROWS + 1 })` currently
  hard-limits the parser. Raising `sheetRows` to `XLSX_MAX_ROWS + 1` is required
  or the lifted per-sheet cap would silently still truncate at 10k. Confirmed the
  parse limit and the `slice` limit are TWO separate places that both reference
  `MAX_ROWS` — both must move together (called out in ITEM-6).
- **FilePanel (ITEM-8)** — comment-only; no behavior change. No breakage.
- **Gallery (ITEM-9)** — additive new seeded surfaces; existing surfaces
  untouched. Regenerated manifests are checked by `check:gallery-coverage` +
  `check:state-matrix`; a stale manifest fails `npm run check` (caught in
  phase 8), so the regen step is mandatory but self-verifying.

## Pattern conformance

- ITEM-1 mirrors `pdf/body.tsx` (all slots mounted, `contentVisibility:auto` +
  `containIntrinsicSize`, IntersectionObserver with `rootMargin`, eager-load the
  first slots, observe `[data-*-index]` against the OverlayScrollbars viewport).
  Conforms — same pattern, text chunks instead of PDF pages.
- ITEM-1 reuses the EXISTING `lineNumberTransformer` + `codeToHtml` + lazy
  `loadShiki` + theme selection + plain-text fallback, applied per-chunk with a
  line-number offset. No new highlighter path invented ([[feedback_match_existing_patterns]]).
- ITEM-5/6 change is a deletion of the head-cap; the kit `Table` virtualization
  + `table-view-core` sort/filter are untouched. Conforms.
- ITEM-9 mirrors `DelimitedViewerDemo`/`XlsxViewerDemo` + their `seededSurfaces`
  entries. Conforms.
- Naming: constants stay UPPER_SNAKE (`RAWCODE_MAX_LINES`, `XLSX_MAX_ROWS`,
  `RAWCODE_CHUNK_LINES`); app name `ziee` unaffected ([[feedback_naming_ziee]]).

## Migration collisions

- None. No SQL migration is added or touched; latest committed migration is
  `00000000000132`. This is a frontend-only feature. The "OOM backstop" caps are
  client-side constants, not DB.

## OpenAPI regen

- Not required. No Rust request/response type changes, no new/changed REST
  endpoints. The tabular/text viewers consume already-existing file-content
  endpoints (`useFileTextContent`, `getFileBinaryContent`,
  `useResourceLinkContent`) with no shape change. Therefore this diff does NOT
  touch `openapi.json` / `api-client/types.ts`, and the phase-3/phase-8 frontend
  gate treats it as UI work purely because of the `.tsx` edits (correct).
- BOTH-workspace obligation (ITEM-10) is about `npm run check` parity, NOT
  openapi regen: `src-app/desktop/ui` consumes `src-app/ui/src` via the
  `@/* → ../../ui/src/*` alias (verified in `desktop/ui/tsconfig.json` +
  `include: [..., "../../ui/src"]`), so the same source is typechecked twice and
  must pass under both configs. No duplicated code.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — mirrors `pdf/body.tsx` observer + `content-visibility`
  slot pattern and reuses the existing Shiki transformer per-chunk; highest-risk
  but well-scoped, covered by find/word-wrap/scroll e2e.
- **ITEM-2** — verdict: PASS — a constant change plus banner-threshold move; the
  10 MB byte cap upstream still bounds the fetch, the raised line cap is the DOM
  OOM guard for the newline-storm case.
- **ITEM-3** — verdict: PASS — find is preserved by construction (all line text
  stays in the DOM); `useFindInDocument`'s MutationObserver already re-matches on
  the plain→highlight swap (its comment states it survives `content-visibility`).
  Hardening only if a regression surfaces; contract left intact.
- **ITEM-4** — verdict: PASS — word-wrap is CSS-only (`raw-code-wrap`); applies to
  plain + highlighted chunks identically. Reserved-height must be wrap-aware
  (noted); low risk.
- **ITEM-5** — verdict: PASS — deletion of the CSV head-cap; virtualization +
  full-array sort/filter already exist; byte cap bounds rows 1:1. Client-side, no
  server paging.
- **ITEM-6** — verdict: CONCERN — xlsx decompresses, so the byte cap does NOT
  bound row count; the raised `XLSX_MAX_ROWS` per-sheet cap is a REAL OOM guard,
  not cosmetic. Must move BOTH the `sheetRows` parse limit and the `slice` limit.
  Resolved by keeping a raised (not removed) cap + banner; flagged in DECISIONS.
- **ITEM-7** — verdict: PASS — assess-and-document; the RENDERED markdown path
  keeps the byte cap (streamdown has no windowing seam), RAW markdown inherits
  ITEM-1 windowing via `RawCodeView`. Comment-only code change.
- **ITEM-8** — verdict: PASS — comment-only; keeps the 10 MB byte backstop as the
  single upstream OOM bound. No behavior change.
- **ITEM-9** — verdict: CONCERN — requires regenerating the gallery/state-matrix
  manifests (`gen:gallery-coverage`, `gen:state-matrix`) after adding surfaces or
  `npm run check` fails on drift. Mandatory but self-verifying in phase 8.
- **ITEM-10** — verdict: PASS — single-source via alias; obligation is dual
  `npm run check`, no duplication. Verified desktop consumes `ui/src`.

No `BLOCKED` verdicts. The two `CONCERN`s (xlsx OOM cap, gallery regen) are
resolved procedurally (keep a raised xlsx cap; run the gen scripts) and are
surfaced in DECISIONS.
