# TESTS — pdf-viewer-architecture

Every ITEM is covered by ≥1 TEST. Pure logic (zoom/search/nav helpers, mockApi
binary, module mapping) gets `unit`; backend endpoint gets `integration`; every
user-visible viewer flow gets an `e2e` spec. Mock only the external boundary —
the e2e specs drive the real renderer end-to-end (real PDF bytes → real pdfjs).

## Backend

- **TEST-1** (tier: integration) [covers: ITEM-1, ITEM-2] file: `src-app/server/tests/file/pdf_raw_test.rs` — asserts: upload `test_data/multipage.pdf`, `GET /files/{id}/raw` returns 200 with `Content-Type: application/pdf`, `Content-Disposition: inline`, and a body byte-identical to the uploaded PDF (route is registered ⇒ ITEM-2; handler serves inline original bytes ⇒ ITEM-1).
- **TEST-2** (tier: integration) [covers: ITEM-1] file: `src-app/server/tests/file/pdf_raw_test.rs` — asserts: user B requesting user A's file `GET /files/{id}/raw` gets 404 (owner-scoped), and a caller lacking `files::preview` gets 403 (perm gate).

## Frontend — pure-logic unit (node --test)

- **TEST-3** (tier: unit) [covers: ITEM-7] file: `src-app/ui/src/modules/file/viewers/pdf/zoom.test.ts` — asserts: `fitWidth`/`fitPage` compute scale = viewport/pageDim (fit-width uses width, fit-page uses min of width/height ratio); zoom-in/out step clamps to the min/max bounds; actual-size = 1.0.
- **TEST-4** (tier: unit) [covers: ITEM-8] file: `src-app/ui/src/modules/file/viewers/pdf/search.test.ts` — asserts: match enumeration over per-page joined text finds all case-insensitive occurrences in document order, `next`/`prev` cycle wrap-around correctly, current-match index maps back to `{page, charStart, charEnd}`, and empty/no-match query yields 0 matches with a null current.
- **TEST-5** (tier: unit) [covers: ITEM-6] file: `src-app/ui/src/modules/file/viewers/pdf/nav.test.ts` — asserts: `pageForScroll(offsets, scrollTop)` returns the page whose band contains the scroll position (the "Page N of M" current page), and `jumpTarget(page)` clamps to `[1, numPages]`.
- **TEST-6** (tier: unit) [covers: ITEM-11] file: `src-app/ui/src/dev/gallery/mockApi.binary.test.ts` — asserts: a binary cassette entry for `/files/{id}/raw` produces a `Response` with `application/pdf` content-type whose `arrayBuffer()` equals the fixture bytes (proves the new binary-response path returns bytes, not JSON).
- **TEST-7** (tier: unit) [covers: ITEM-10] file: `src-app/ui/src/modules/file/viewers/pdf/module.test.ts` — asserts: the exported `viewers[]` maps the `application/pdf`/`pdf` entry's `body` to `PdfJsBody` and the DOCX/RTF/ODT entry's `body` to the legacy `PdfBody` (module split routes correctly).

## Frontend — e2e (Playwright, real app)

- **TEST-8** (tier: e2e) [covers: ITEM-3, ITEM-4, ITEM-5, ITEM-9, ITEM-10] file: `src-app/ui/tests/e2e/file-viewer/pdf-viewer.spec.ts` — asserts: after uploading a multi-page PDF and opening its preview, pdfjs loads (worker chunk fetched, no console error), page 1 renders a `<canvas>` with a positioned text layer, the pdfjs toolbar (not the office image body) is shown, the selected page text is selectable/copyable, and the truncation banner is absent.
- **TEST-9** (tier: e2e) [covers: ITEM-6] file: `src-app/ui/tests/e2e/file-viewer/pdf-viewer.spec.ts` — asserts: the indicator shows "Page 1 of N"; clicking next advances to page 2 (indicator + scroll), prev returns, and typing a page number in jump-to-page scrolls that page into view and updates the indicator.
- **TEST-10** (tier: e2e) [covers: ITEM-7] file: `src-app/ui/tests/e2e/file-viewer/pdf-viewer.spec.ts` — asserts: zoom-in increases the rendered canvas pixel width, zoom-out decreases it, fit-width fills the viewport width, and actual-size returns to 100%.
- **TEST-11** (tier: e2e) [covers: ITEM-8] file: `src-app/ui/tests/e2e/file-viewer/pdf-viewer.spec.ts` — asserts: opening find, typing a term present in the PDF shows an "x of N" count with visible highlight(s), next/prev move the active highlight and scroll to it, and a term not present shows "0" matches.
- **TEST-12** (tier: e2e) [covers: ITEM-11] file: `src-app/ui/tests/e2e/file-viewer/pdf-viewer-gallery.spec.ts` — asserts: the gallery `overlay-file-preview-drawer` (PDF fixture) renders the loaded PDF page offline via the mockApi binary route with zero console errors / failed requests (backs the gallery loaded/find-open state cells that `check:state-matrix` + `gate:ui` enforce).

## Coverage map (every ITEM → ≥1 TEST)

- ITEM-1 → TEST-1, TEST-2
- ITEM-2 → TEST-1
- ITEM-3 → TEST-8
- ITEM-4 → TEST-8
- ITEM-5 → TEST-8
- ITEM-6 → TEST-5, TEST-9
- ITEM-7 → TEST-3, TEST-10
- ITEM-8 → TEST-4, TEST-11
- ITEM-9 → TEST-8
- ITEM-10 → TEST-7, TEST-8
- ITEM-11 → TEST-6, TEST-12
