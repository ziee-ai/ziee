# TEST_RESULTS — file-viewer-virtualization

Scoped to the touched areas (frontend-only diff). Both UI workspaces gated.

## Frontend gate (per touched workspace)

- npm run check (ui): PASS
- npm run check (desktop/ui): PASS

`npm run check` chains tsc + biome guardrails + lint:colors + lint:settings-field
+ check:kit-manifest + check:testid-registry + check:design-spec +
check:gallery-coverage + check:gallery-crawl + check:state-matrix +
check:overlay-registry. Green in both `src-app/ui` and `src-app/desktop/ui`
(desktop typechecks the shared `../../ui/src` via the `@/*` alias).

## UI evaluator gate (touched gallery surfaces)

Scoped runtime-health over the two new surfaces (`seeded-delimited-viewer-large`,
`seeded-rawcode-large`) × light/dark = 4 cells: **0 HIGH (gating), 0 MEDIUM, 2
LOW**. The 2 LOW are informational `spacing-grid` drift (6px/10px) inherited from
the kit Table's 2px half-steps (never gating; not introduced by this diff). Zero
console errors / uncaught exceptions / failed requests / AA-contrast failures on
the touched surfaces.

## Unit tier (node:test)

- **TEST-1**: PASS — chunkLines contiguous/ordered chunks, byte-exact round-trip, startLine offset, empty/one-line → one chunk.
- **TEST-2**: PASS — applyLineCap pass-through below / slice-to-cap above; RAWCODE_MAX_LINES ≫ 10k.
- **TEST-3**: PASS — chunkReservedHeight wrap-aware (wrap > no-wrap) + linear.
- **TEST-4**: PASS — parseDelimitedText returns ALL >10k rows (truncated:false); real truncated:true slice via injected cap (production path, not a re-implemented predicate).
- **TEST-5**: PASS — shared capRows (used by XlsxBody) pass-through/slice/at-cap; XLSX_MAX_ROWS raised backstop; retired 10k no longer truncates.
- **TEST-6**: PASS — escapeHtml round-trip + plain/tokenized line textContent equality (plain→highlight swap preserves find text).

(20 assertions across chunking.test.ts + highlight-swap.test.ts + parse-cap.test.ts; 20 pass / 0 fail.)

## E2E tier (Playwright)

- **TEST-7**: PASS — large-text-viewer.spec.ts "renders past 10k … windowed chunk slots + scroll": 25k-line file, no truncation banner, >1 chunk slot, last-line sentinel present in DOM + visible after scroll.
- **TEST-8**: PASS — large-text-viewer.spec.ts "find-in-document spans the whole file": MARKER on lines 5 / 12000 / 24000 → find counts "1 of 3", next→"2 of 3"→"3 of 3"→wrap "1 of 3" (matches past the retired 10k cap + initial window). (Passed on rerun after the assertion-string fix; the feature counted all 3 on the first run too.)
- **TEST-9**: PASS — large-text-viewer.spec.ts "word-wrap toggle works under windowing": wrap off→long line overflows; on→wraps (no h-overflow) + gutter intact; back off→overflows.
- **TEST-10**: PASS — large-table-viewer.spec.ts "CSV full dataset, filter spans rows past 10k": 15k-row CSV, no truncation banner, readout "15,000", filter for a category on row 14,000 → "Showing 1 of 15,000" + row visible. (Passed on rerun; the first-run failure was an infra "backend failed to start on port" flake under high host load, not a product defect.)
- **TEST-11**: PASS — large-table-viewer.spec.ts "XLSX full sheet, filter spans rows past 10k": 12k-row generated xlsx, no per-sheet banner, readout "12,000", filter for a value on row 11,000 → "Showing 1 of 12,000" + row visible.
- **TEST-12**: PASS — large-table-viewer.spec.ts "Markdown rendered keeps content; raw uses the windowed viewer": large md renders (heading visible), raw-mode toggle → raw-code-view visible.
- **TEST-13**: PASS — visual/large-viewer-gallery.spec.ts TEST-13a (large CSV row-virtualized, full count, no banner, no pageerror) + TEST-13b (large raw-code windowed, >1 chunk, on-view Shiki highlight colored spans present in the dev gallery, no pageerror).
- **TEST-14**: PASS — the single-source `ui/src` viewer components (consumed by desktop via the `@/*` alias) render correctly in the real viewer path (exercised by TEST-7..12); workspace parity enforced by `npm run check (ui): PASS` + `npm run check (desktop/ui): PASS` above.

All 14 enumerated tests PASS.
