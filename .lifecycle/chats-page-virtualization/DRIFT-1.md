# DRIFT-1 — implementation vs plan (round 1)

Reconciling the shipped code against PLAN.md / DECISIONS.md / TESTS.md.

- **DRIFT-1.1** — verdict: impl-wins — **`scrollMargin` dropped (DEC-6).** The plan
  (ITEM-4 / DEC-6) called for a measured `scrollMargin`. In the actual DOM the
  virtual container sits at the scroll viewport's content top (M ≈ 0): the
  `DivScrollY` `!py-3` is host padding OUTSIDE the OverlayScrollbars viewport, the
  bulk-actions bar is above the scroller, and the in-viewport wrappers add no top
  padding. So `scrollMargin` is a no-op and a mis-measured one would be riskier
  than omitting it — matching `MessageList.tsx` (which sets none). DECISIONS.md
  DEC-6 amended with the rationale; ITEM-4's remaining wiring (scroller ref,
  `scrollerReady`, desktop/mobile split) is implemented as planned. Correctness is
  RUN-proven by TEST-4/5 (rows mount at the right offsets). Resolved.

- **DRIFT-1.2** — verdict: impl-wins — **pure metrics factory extracted to a `.ts`
  util.** ITEM-6's metrics were planned inline in the component; the `node:test`
  loader can execute `.ts` but not `.tsx` (JSX), so the pure `makeChatListMetrics`
  factory lives in `core/utils/chatListMetrics.ts` (unit-tested by TEST-3) and the
  component imports it. Strictly better (pure logic unit-testable, no render).
  TESTS.md TEST-3 file path updated. Resolved.

- **DRIFT-1.3** — verdict: impl-wins — **`measuredHeightCache.ts` NOT edited.**
  PLAN "Files to touch" listed it as a possible edit (generalize keys). It was
  already fully id-generic (DEC-2), so it is imported AS-IS with ZERO edit — even
  more surgical than planned, and the message path is provably byte-identical.
  Only its TEST file gained conversation-reuse cases (TEST-2). Resolved.

- **DRIFT-1.4** — verdict: impl-wins — **gallery-based e2e moved to the VISUAL
  suite; DOM-walk scroll on the real path.** The window/scroll/no-jank specs
  (TEST-4/5/6) run under `playwright.visual.config.ts` (dev-mode Vite → `/gallery.html`
  + `import.meta.env.DEV` metrics live), NOT the default e2e config (which serves a
  PROD `vite build` where the gallery + DEV metrics are absent). TESTS.md TEST-4/5/6
  file path updated to `tests/e2e/visual/...`. The real-path spec (TEST-7/8) stays
  under the default config and targets the OverlayScrollbars viewport by walking up
  from a card to its scrollable ancestor (no viewport testid needed). DEC-10's
  scroller testid (`chat-conversation-list-scroll`) is therefore added on the
  gallery demo's scroll box (where the e2e drives scrollTop directly); the real
  path uses the DOM-walk. Resolved.

- **DRIFT-1.5** — verdict: none — **new file `chatListMetrics.ts` added** (not in
  the original Files-to-touch). A consequence of DRIFT-1.2; recorded for the audit
  trail. No conflict.

- **DRIFT-1.6** — verdict: none — **new gallery demo split into wide + narrow
  surfaces** (`seeded-conversation-list-long` + `-narrow`) as ITEM-7 planned; the
  component `coverage.ts` entry + regenerated manifests (`testid-registry`,
  `gallery-coverage`, `state-matrix`, `gallery-crawl`, `overlay-registry`) keep
  `npm run check` green. As planned.

**Unresolved drifts:** 0
