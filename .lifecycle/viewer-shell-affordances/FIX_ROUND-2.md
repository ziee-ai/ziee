# FIX_ROUND-2 — convergence round

A fresh blind agent (diff-only, no prior context) re-reviewed the round-1-fixed
diff across all angles, focusing on the three reworked mechanisms (image pan
rAF/ResizeObserver, the Ctrl-F focus-scoped document listener + per-instance
highlight names, and the debounced find rebuild + stable observer).

Verdict: **no new defect survives scrutiny.** The agent verified point-by-point:

- **image/body.tsx** — `requestAnimationFrame` returns a positive integer per
  spec so the `rafRef` sentinel is sound (no double-schedule); the ResizeObserver
  + re-clamp effects list `view.scale`/`view.mode` in deps so `overflow()`'s
  closure is never stale; the CSS `transform` is visual-only on an
  `overflow-hidden` container so the RO can't feedback-loop; `endDrag` cancels the
  pending rAF and commits exactly once.
- **FindableRegion.tsx** — the focus-scoped Ctrl-F gate is correct for the app's
  one-viewer-at-a-time / modal-drawer layouts (last-registered == topmost, modal
  focus-trap prevents the divergent case); per-instance names via the guarded
  `useRef` are stable; the listener is added on mount and removed when the
  registry empties (StrictMode nets one entry); `host` computed once at mount is
  fine (viewers aren't re-parented).
- **useFindInDocument.ts** — the debounce timer is cleared on cleanup; the
  observer uses the stable `rebuildRef` (no per-keystroke teardown); `paintActive`
  after rebuild covers the count/index-unchanged case; the Highlight API mutates
  no DOM so there's no observer feedback loop.

**New confirmed findings:** 0
