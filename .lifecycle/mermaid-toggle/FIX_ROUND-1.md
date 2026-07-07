# FIX_ROUND-1 — resolve confirmed blind-audit findings

Merged the 3 blind reviewers' ledger. Fixed every CONFIRMED finding; recorded
each rejection with a defensible rationale (a rejected finding is not silently
dropped — see LEDGER.jsonl `status:"rejected"`).

## Fixed

- **state-management (empty code → perpetual placeholder)** — added an `isEmpty`
  branch: whitespace-only source now renders a distinct "Empty diagram" state
  (`data-testid="mermaid-empty"`) instead of an endless "Rendering diagram…".
- **concurrency (same-id collision) + error-handling (stray-node cleanup)** —
  the mermaid temp id is now unique per render (`${baseId}-${++renderSeq}`), so a
  stale in-flight render can't collide with its successor; error cleanup removes
  BOTH `d<id>` and `<id>` stray nodes.
- **concurrency/perf (revoke aborts download)** — `URL.revokeObjectURL` is now
  deferred (`setTimeout(…, 0)`) so the browser finishes reading the blob first.
- **a11y (unlabeled graphic; no live region)** — the diagram container gets
  `role="img"` + `aria-label="Mermaid diagram"`; the error block gets
  `role="alert"`; the rendering placeholder gets `role="status"`.
- **tests-quality (download not proven)** — TEST-5 now reads the downloaded file
  via `download.path()` + `readFile` and asserts the bytes contain `<svg`.

## Rejected (with rationale)

- **security (dangerouslySetInnerHTML XSS)** — `securityLevel:'strict'` runs
  mermaid's bundled DOMPurify; this is the SAME trust level as the Streamdown
  built-in mermaid path being replaced (no new surface). Not a regression.
- **concurrency (global-init theme interleave)** — the app has ONE global theme,
  so `isDark` is identical for every instance at any instant; the divergent-theme
  race cannot occur.
- **perf (eager render in source mode)** — intentional (DEC-6): instant toggling
  AND download-SVG-while-viewing-source both require the SVG to be kept ready.
- **tests-quality (clipboard Chromium-only)** — the visual Playwright project runs
  a single Desktop Chrome project where clipboard-read is granted and reliable.

## Verification

- `tsc --noEmit` (ui + desktop/ui): exit 0.
- `npm run check` (ui): exit 0 (incl. regenerated testid registry for the new
  `mermaid-empty` testid).

**New confirmed findings:** 5
