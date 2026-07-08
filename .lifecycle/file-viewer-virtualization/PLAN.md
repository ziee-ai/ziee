# PLAN — file-viewer-virtualization

Lift the per-viewer 10k truncation caps and add on-demand windowing so the file
viewers scale to LARGE files (long logs / JSON / source, big CSV/TSV, large
spreadsheets), mirroring the PDF viewer's already-proven page-on-demand model
(all slots mounted in the DOM + `content-visibility:auto` + heavy content loaded
lazily via an IntersectionObserver). The FilePanel 10 MB byte cap stays as the
outer OOM backstop.

Frontend-only. No backend / OpenAPI / migration changes (see PLAN_AUDIT). Both UI
workspaces (`src-app/ui` + `src-app/desktop/ui`) — the file viewers are mirrored
into desktop/ui, so every source edit is applied to BOTH.

## Items

- **ITEM-1**: Chunk-window the TEXT/CODE Shiki highlight in `RawCodeView`. Keep
  the current all-lines-present DOM structure (`.line`/`.line-number`/`.line-code`
  grid + `content-visibility:auto`) that find-in-document's TreeWalker depends on,
  but split the source into fixed-size line chunks; render each chunk as a
  container slot (reserved height via `contain-intrinsic-size`) showing PLAIN
  (escaped) text by default, and highlight only the chunks scrolled into view (+
  a prefetch margin) via an IntersectionObserver — exactly mirroring
  `pdf/body.tsx`'s page-slot + observer pattern. Highlighted chunk HTML is cached
  so re-scroll never re-highlights.
- **ITEM-2**: Lift the TEXT/CODE `MAX_LINES = 10_000` truncation. Replace the low
  head-cap with a high OOM-backstop line cap (`RAWCODE_MAX_LINES = 300_000`) that
  exists only to bound the pathological "10 MB of newlines → millions of DOM rows"
  case the byte cap can't catch; below it the FULL file renders (windowed). The
  truncation banner only appears at the new backstop, not at 10k.
- **ITEM-3**: Preserve find-in-document (F2) over the FULL windowed text.
  Because ITEM-1 keeps every line's text node in the DOM (plain or highlighted,
  same text content), `useFindInDocument`'s TreeWalker already sees the whole
  file; a match in an offscreen chunk is reached via the existing
  `scrollIntoView`, which reveals the chunk (all chunks are laid-out via
  `content-visibility:auto`) and triggers its lazy highlight. Verify + harden:
  the MutationObserver rebuild (fired when a chunk swaps plain→highlighted) must
  not drop the active match, and highlight-swap must not change text content
  (only wrap tokens), so ranges stay valid.
- **ITEM-4**: Preserve the word-wrap toggle over the windowed text. The
  `raw-code-wrap` CSS (grid collapses to `44px 1fr`, `white-space: pre-wrap`)
  must apply identically to both plain-text and highlighted chunk slots; because
  wrap changes line height, chunk reserved-height (`contain-intrinsic-size`) is
  computed per wrap-mode so scrollbar geometry stays sane in both.
- **ITEM-5**: Lift the TABULAR CSV/TSV `MAX_ROWS = 10_000` head-cap in
  `DelimitedTable` so the FULL dataset is parsed into `dataSource`; the kit
  Table's `virtualized` path (useVirtualizer) already mounts only visible rows,
  and sort/filter (`applySort`/`applyFilter` in `table-view-core`) operate over
  the whole in-memory array — so sort/filter now correctly span the entire file,
  not a 10k head. Client-side (no server paging); the 10 MB byte cap bounds CSV
  rows 1:1.
- **ITEM-6**: Lift the TABULAR XLSX `MAX_ROWS = 10_000` head-cap in `XlsxBody`
  (both the `sheet_to_json` slice AND the `XLSX.read({ sheetRows })` parse limit)
  to a raised OOM-backstop (`XLSX_MAX_ROWS = 200_000`). XLSX is compressed, so
  the 10 MB byte cap does NOT bound decompressed row count — the raised per-sheet
  cap is the real OOM guard here (distinct from CSV, which the byte cap bounds).
  Full sheet (up to the backstop) is parsed + virtualized; sort/filter span it.
- **ITEM-7**: MARKDOWN — assess + DOCUMENT. Streamdown exposes no block-windowing
  seam and reliable markdown block-splitting (nested tables/lists/code fences) is
  not clean; the RENDERED markdown path keeps the FilePanel byte cap as its
  boundary. Add a code comment in `markdown/body.tsx` recording the rationale.
  The markdown RAW mode already delegates to `RawCodeView`, so it inherits the
  ITEM-1/2 windowing for free — call that out too.
- **ITEM-8**: Keep the FilePanel `PREVIEW_SIZE_LIMIT_BYTES = 10 MB` byte cap as
  the outer OOM backstop, unchanged. Confirm (comment) that after ITEM-1..6 the
  per-viewer caps are OOM guards, not preview-truncation UX, and the 10 MB byte
  cap remains the single upstream bound that prevents fetching a pathological
  file at all.
- **ITEM-9**: Gallery coverage — add large-file gallery surfaces so the new
  windowing/lifted-cap render states are covered by the state-matrix + runtime
  gate: a `RawCodeView` large-text demo (thousands of lines) and a
  `DelimitedTable` large-CSV demo (>10k rows). Mirror the existing
  `DelimitedViewerDemo`/`XlsxViewerDemo` seeded-surface pattern in `TableDemos`
  + `seededSurfaces`, then regenerate the gallery/state-matrix manifests.
- **ITEM-10**: Parity across BOTH UI workspaces. `src-app/desktop/ui` does NOT
  keep its own copy of the file module — it consumes `src-app/ui/src` through the
  `@/* → ../../ui/src/*` tsconfig path alias, so every viewer edit is
  single-source in `src-app/ui`. The obligation is therefore: (a) the edits must
  typecheck under desktop's tsconfig (its `tsc` also includes `../../ui/src`),
  and (b) `npm run check` must pass in BOTH `src-app/ui` and `src-app/desktop/ui`
  (the desktop gallery is separate and does not reference the file viewers, so
  the ITEM-9 gallery surfaces are added to the `ui` gallery only). No code is
  duplicated into `desktop/ui`.

## Files to touch

- `src-app/ui/src/modules/file/viewers/shared/RawCodeView.tsx` (ITEM-1..4)
- `src-app/ui/src/modules/file/viewers/shared/chunking.ts` NEW — pure
  chunk/cap/height/escape helpers extracted for DOM-free unit testing (ITEM-1..4)
- `src-app/ui/src/modules/file/viewers/tabular/DelimitedTable.tsx` (ITEM-5)
- `src-app/ui/src/modules/file/viewers/tabular/parse.ts` NEW — pure delimited
  parse + `DELIMITED_MAX_ROWS`/`XLSX_MAX_ROWS` constants extracted for DOM-free
  unit testing (ITEM-5/6)
- `src-app/ui/src/modules/file/viewers/tabular/XlsxBody.tsx` (ITEM-6)
- `src-app/ui/src/modules/file/viewers/markdown/body.tsx` (ITEM-7, comment only)
- `src-app/ui/src/modules/file/components/FilePanel.tsx` (ITEM-8, comment only)
- `src-app/ui/src/modules/file/viewers/shared/find/useFindInDocument.ts` (ITEM-3,
  only if hardening is needed; ideally untouched)
- `src-app/ui/src/dev/gallery/TableDemos.tsx` (ITEM-9)
- `src-app/ui/src/dev/gallery/seededSurfaces.tsx` (ITEM-9)
- Regenerated gallery manifests (`galleryCoverage.generated.ts`,
  `stateMatrix.generated.ts`, `STATE_MATRIX.md`) via `npm run gen:*` (ITEM-9)
- New unit tests:
  `src-app/ui/src/modules/file/viewers/shared/chunking.test.ts` (ITEM-1/2),
  `src-app/ui/src/modules/file/viewers/tabular/*` cap tests (ITEM-5/6)
- `src-app/ui/tests/e2e/file/helpers.ts` — add `seedProjectBinaryFile` (real
  xlsx-binary upload for the large-table e2e), mirroring `seedProjectImage`
- `src-app/ui/src/dev/gallery/stateCoverage.ts` — add a skip entry for the
  RawCodeView effect-guard state the detector flags (ITEM-9)
- New e2e specs under `src-app/ui/tests/e2e/file/` (large-text-viewer,
  large-table-viewer) + `tests/e2e/visual/large-viewer-gallery.spec.ts` — see TESTS.md
- No `src-app/desktop/ui/**` source edits (single-source via alias, ITEM-10);
  only its `npm run check` must stay green.

## Patterns to follow

- **On-demand windowing** → `src-app/ui/src/modules/file/viewers/pdf/body.tsx`:
  all slots mounted, `contentVisibility:auto` + `containIntrinsicSize`,
  IntersectionObserver with `rootMargin` prefetch, eager-load the first slots,
  observe `[data-*-index]` elements against the OverlayScrollbars viewport. The
  text chunk-slot loop mirrors the PDF page-slot loop 1:1.
- **Shiki highlight + line-number transformer** → the EXISTING
  `RawCodeView.tsx` (`lineNumberTransformer`, `codeToHtml`, lazy `loadShiki`,
  theme selection, plain-text fallback) — reused per-chunk with a global
  line-number offset closure; do NOT invent a new highlighter path.
- **Row virtualization + full-array sort/filter** → the EXISTING
  `DelimitedTable.tsx` / `XlsxBody.tsx` (`virtualized` kit `Table`,
  `VIRTUALIZE_ROW_THRESHOLD`, `onViewChange`) — the change is only removing the
  `slice(0, MAX_ROWS)` head-cap, not new table code.
- **find-in-document** → the EXISTING `useFindInDocument.ts` /
  `FindableRegion.tsx` — TreeWalker over DOM text nodes + CSS Custom Highlight
  API; leave its contract intact (that is why ITEM-1 keeps all lines in the DOM).
- **Gallery seeded surface** → the EXISTING `DelimitedViewerDemo` /
  `XlsxViewerDemo` in `TableDemos.tsx` + their `seededSurfaces.tsx` entries.
- **e2e file seeding** → `tests/e2e/file/find-in-document.spec.ts` +
  `tests/e2e/file/helpers.ts` (`seedProjectFile` real upload → `openPreviewDrawer`).
