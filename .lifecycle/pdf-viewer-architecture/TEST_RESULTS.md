# TEST_RESULTS — pdf-viewer-architecture (Phase 8)

Scoped to the touched areas (backend `file::` module + both UI workspaces).

## Backend integration (`cargo test --test integration_tests file::pdf_raw -- --test-threads=1`)

- **TEST-1**: PASS — `test_get_raw_returns_inline_pdf_bytes` (200 + `application/pdf` + `Content-Disposition: inline` + body byte-identical to the uploaded PDF).
- **TEST-2**: PASS — `test_get_raw_is_owner_scoped` (cross-user request → 404; the `files::download` gate is the standard `RequirePermissions` extractor, documented in-test).

(`test result: ok. 2 passed; 0 failed`.)

## Frontend unit (`node --test`)

- **TEST-3**: PASS — `zoom.test.ts` (zoom-step ladder + clamps).
- **TEST-5**: PASS — `nav.test.ts` (page clamp/parse + `canPrevPage`/`canNextPage` boundaries).
- **TEST-6**: PASS — `mockApi-binary.test.ts` (binary Response returns fixture bytes, not JSON).

(11 tests pass across the three files.)

## Frontend static gate

- npm run check (ui): PASS
- npm run check (desktop/ui): PASS

(`tsc` + biome guardrails + `lint:colors`/`lint:settings-field` + `check:kit-manifest`/`check:testid-registry`/`check:design-spec`/`check:gallery-coverage`/`check:gallery-crawl`/`check:state-matrix`/`check:overlay-registry` — the gallery-crawl renders the real `PdfJsBody` with the fixture PDF offline with zero console errors, which is the runtime-health/UI-evaluator pass for this surface.)

## Frontend e2e (`playwright test tests/e2e/visual/pdf-viewer.spec.ts --config=playwright.visual.config.ts --workers=1`)

Drives the real `PdfJsBody` (real pdfjs-dist PDFViewer + canvas + text layer +
PDFFindController) via the gallery `overlay-file-preview-drawer` surface.

- **TEST-8**: PASS — pdfjs renders (canvas), the pdfjs toolbar (not the office image body) is shown, a text layer with real spans exists (selectable), no truncation banner, no page errors.
- **TEST-9**: PASS — "of 3" indicator; next → page 2, prev → page 1, jump input → page 3.
- **TEST-10**: PASS — zoom-in enlarges the rendered canvas width; fit-width + actual-size apply cleanly.
- **TEST-11**: PASS — find "ZIEEFINDABLE" → "of 1" count + a visible `.textLayer .highlight`; a missing term → "0 of 0".
- **TEST-12**: PASS — the gallery renders the loaded PDF offline via the mockApi binary route with zero console errors / page errors / failed requests.

(`5 passed`.)

## Summary

All Phase-3 TEST-IDs (1, 2, 3, 5, 6, 8–12) PASS; both UI workspaces pass
`npm run check`. No test was skipped or `#[ignore]`d.
