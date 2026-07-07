# FIX_ROUND-1 — fix the Phase-6 ledger + first re-audit

## Part A — fixed every confirmed Phase-6 ledger finding (16)

- image zoom-out re-clamp; drag session (measure-once) + cursor via `dragging` state; keyboard arrow-pan; a11y focusable pan region.
- find: `rebuild` now repaints the active highlight; debounced rebuild; stable MutationObserver via `rebuildRef` (no per-keystroke churn); extracted `locateSegment` (offset.ts) + unit test (TEST-12); strengthened `highlightSupported.test.ts` (stub-driven supported=true branch).
- FindableRegion: module-level region registry + document Ctrl-F listener (fixes the dead keyboard-open path); focus restored to the region on close.
- chrome: CopySelectionButton gated to a selection inside `[data-testid="file-findable-region"]`; FindButton tooltip platform-aware (⌘F on macOS).
- image header Segmented onChange uses `Stores.File.__state`.
- image-zoom e2e gains a real drag + keyboard-pan assertion.

## Part B — first re-audit (fresh blind agents over the fixed diff) → NEW findings, all fixed

- **FR1.1** (concurrency) `image/body.tsx`: direct `img.style.transform` write during drag was reverted by any interleaved re-render (snap-back mid-drag). → Fixed: pan now commits through React state, **coalesced to one `setState` per animation frame** (rAF), so React owns the transform and there's no per-event render storm.
- **FR1.2** (error-handling) `image/body.tsx`: `endDrag` on both pointerup + pointerleave called `releasePointerCapture` with a stale pointerId → uncaught DOMException. → Fixed: early-return when no active drag; release only when `hasPointerCapture(pointerId)`.
- **FR1.3** (correctness/patterns) `FindableRegion.tsx`: the document Ctrl-F listener hijacked native find page-wide whenever a viewer was merely visible. → Fixed: intercept ONLY when focus is unclaimed (body/null) or inside the viewer's host surface (dialog / full-page container); otherwise fall through to native find.
- **FR1.4** (state-management) `FindableRegion.tsx`: highlight names keyed by `fileId` still clobbered when the same file is open in two regions (drawer + full page). → Fixed: names are now per-INSTANCE (ident-safe random suffix).
- **FR1.5** (correctness) `image/body.tsx`: pan never re-clamped on container/window resize. → Fixed: a `ResizeObserver` on the container re-clamps translate on resize (actual mode).
- **FR1.6** (a11y) `image/body.tsx`: `outline-none` with no focus-visible replacement. → Fixed: `focus-visible:ring-2 ring-ring ring-inset`.
- **FR1.7** (a11y) `image/body.tsx`: `role="img"` made the pannable widget presentational. → Fixed: `role="group"` + `aria-roledescription="Pannable image"`.
- **FR1.8** (a11y) `image/body.tsx`: arrow-key `preventDefault` ran before the overflow check (scroll trap on a non-overflowing image). → Fixed: overflow checked first; no pan → no preventDefault.
- **FR1.9** (tests-quality) `image-zoom.spec.ts`: the 2×2-PNG fixture never overflowed, so the pan assertions would time out. → Fixed: `seedProjectImage` uploads a 640×480 canvas-generated PNG that reliably overflows when zoomed.

**New confirmed findings:** 9
