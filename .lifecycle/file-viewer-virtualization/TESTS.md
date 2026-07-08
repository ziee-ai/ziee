# TESTS — file-viewer-virtualization

Every ITEM is covered by ≥1 TEST. UI-visible items also get a `tier: e2e` spec.
Mock only external boundaries (real file upload for e2e; real components in unit)
([[feedback_no_cosmetic_tests]]). Test file seeding mirrors
`tests/e2e/file/find-in-document.spec.ts` (`seedProjectFile` real upload).

## Unit

- **TEST-1** (tier: unit) [covers: ITEM-1] file: `src-app/ui/src/modules/file/viewers/shared/chunking.test.ts` — asserts: the pure `chunkLines(text, size)` helper splits a source into contiguous line-chunks preserving order, byte-exact join round-trips to the original text, chunk boundaries carry the correct global `startLine` offset, and an empty/one-line input yields one chunk.
- **TEST-2** (tier: unit) [covers: ITEM-2] file: `src-app/ui/src/modules/file/viewers/shared/chunking.test.ts` — asserts: `applyLineCap(lines, RAWCODE_MAX_LINES)` returns `{ lines, truncated:false }` below the cap, `{ truncated:true }` at/above it slicing to exactly the cap, and that `RAWCODE_MAX_LINES` is far above the retired 10k value (regression guard on the lifted cap).
- **TEST-3** (tier: unit) [covers: ITEM-4] file: `src-app/ui/src/modules/file/viewers/shared/chunking.test.ts` — asserts: `chunkReservedHeight(lineCount, wrap)` reserves a larger intrinsic height in wrap mode than in no-wrap mode (wrap-aware `contain-intrinsic-size`) and scales linearly with the chunk's line count.
- **TEST-4** (tier: unit) [covers: ITEM-5] file: `src-app/ui/src/modules/file/viewers/tabular/parse-cap.test.ts` — asserts: `parseDelimitedText` on a CSV with more than 10k data rows returns ALL rows (no head-truncation) and `truncated:false`, and only reports `truncated:true` once the raised OOM-backstop is exceeded.
- **TEST-5** (tier: unit) [covers: ITEM-6] file: `src-app/ui/src/modules/file/viewers/tabular/parse-cap.test.ts` — asserts: the XLSX per-sheet cap constant `XLSX_MAX_ROWS` is the raised backstop (>10k) and that the sheet-truncation predicate (`dataRows.length > XLSX_MAX_ROWS`) fires only above it, not at 10k — i.e. both the parse `sheetRows` limit and the slice limit reference the same raised constant.
- **TEST-6** (tier: unit) [covers: ITEM-3] file: `src-app/ui/src/modules/file/viewers/shared/find/highlight-swap.test.ts` — asserts: swapping a line's rendered content from plain text to Shiki token spans preserves the exact concatenated text (`textContent` equality), so find Ranges built by the TreeWalker stay valid across a chunk's plain→highlight transition (the property ITEM-3 relies on).

## Integration

_(No backend/server integration surface — this is a frontend-only diff. The
cross-component behavior is exercised at the e2e tier against the real FilePanel
→ viewer path, which is the integration boundary for UI viewers.)_

## E2E

- **TEST-7** (tier: e2e) [covers: ITEM-1, ITEM-2, ITEM-8] file: `src-app/ui/tests/e2e/file/large-text-viewer.spec.ts` — asserts: a seeded text file with FAR more than 10k lines (e.g. 25k) opens in the raw-code viewer with NO truncation banner (`file-rawcode-truncated-alert` absent), renders the windowed line DOM, and scrolls to the bottom smoothly revealing the last line's content (windowed highlight applied on scroll) — proving the 10k cap is lifted and windowing works. Small files (a few lines) still render fully with no banner (control).
- **TEST-8** (tier: e2e) [covers: ITEM-3] file: `src-app/ui/tests/e2e/file/large-text-viewer.spec.ts` — asserts: over the SAME 25k-line file, find-in-document (F2 / find button) counts matches of a token that occurs on lines beyond the initial visible window and beyond the old 10k cap, `next` navigates to a match that is off-screen (viewer scrolls it into view), and the count reflects the WHOLE file — proving find spans the full windowed document.
- **TEST-9** (tier: e2e) [covers: ITEM-4] file: `src-app/ui/tests/e2e/file/large-text-viewer.spec.ts` — asserts: toggling word-wrap on the 25k-line file flips `data-word-wrap` on `raw-code-view` and long lines wrap (no horizontal scrollbar) while the windowed line DOM + line-number gutter stay intact — word-wrap preserved under windowing.
- **TEST-10** (tier: e2e) [covers: ITEM-5] file: `src-app/ui/tests/e2e/file/large-table-viewer.spec.ts` — asserts: a seeded CSV with more than 10k rows (e.g. 15k) opens in the tabular viewer with NO truncation banner (`file-delimited-truncated-alert` absent), the row-count readout reflects the FULL row count (>10k), and a filter query matching ONLY a row that sits beyond the old 10k head-cap surfaces that row — proving sort/filter operate over the whole dataset, not a 10k head.
- **TEST-11** (tier: e2e) [covers: ITEM-6] file: `src-app/ui/tests/e2e/file/large-table-viewer.spec.ts` — asserts: a seeded XLSX whose sheet has more than 10k rows opens with NO truncation banner at 10k, the readout reflects the full parsed row count, and sort over the full sheet reorders a value that lives beyond row 10k to the top — proving the xlsx head-cap is lifted to the raised backstop.
- **TEST-12** (tier: e2e) [covers: ITEM-7] file: `src-app/ui/tests/e2e/file/large-table-viewer.spec.ts` — asserts: a seeded large Markdown file renders (rendered mode) with content present and no crash, and its RAW mode toggle renders through the windowed `raw-code-view` (documenting that markdown-rendered keeps the byte cap while raw inherits windowing).
- **TEST-13** (tier: e2e) [covers: ITEM-9] file: `src-app/ui/tests/e2e/visual/large-viewer-gallery.spec.ts` — asserts: the new gallery seeded surfaces (`seeded-delimited-viewer-large`, `seeded-rawcode-large`) mount without runtime errors and render the windowed viewers (drives the gallery coverage the runtime/state-matrix gate needs).
- **TEST-14** (tier: e2e) [covers: ITEM-10] file: `src-app/ui/tests/e2e/file/large-text-viewer.spec.ts` — asserts: the single-source `ui/src` viewer components that desktop consumes via the `@/*` alias render correctly in the real viewer path; workspace parity is additionally enforced in phase 8 by `npm run check (ui)` + `npm run check (desktop/ui)` both passing (the shared source typechecks under both configs).
