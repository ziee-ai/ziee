# FIX_ROUND-2 — fix the round-1 regression → final blind pass

## Fix applied

- **[LOW a11y] focusable container had no visible focus indicator** (`pdfjs-body.tsx`): replaced `focus:outline-none` on the now-focusable (`tabIndex={0}`) PDF scroll container with `outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-inset` — a design-token focus-visible ring (same `ring-ring` token the kit input/select/date-picker components use). Keyboard users tabbing into the PDF region now get a visible focus indicator (WCAG 2.4.7), while mouse focus stays quiet (focus-visible).

## Final blind re-audit

A fresh blind fork reviewer confirmed the focus-ring change is sound (visible
indicator via design-token ring, no regression) and swept the whole PDF-viewer
diff (management.rs, pdfjs.ts, usePdfDocument.ts, pdfjs-body.tsx, nav.ts,
zoom.ts, module.tsx, mockApi.ts, pdf_raw_test.rs) once more for any remaining
substantiated defect. Result: **NO NEW FINDINGS.**

## Verification

- `tsc` clean (both workspaces).

**New confirmed findings:** 0
