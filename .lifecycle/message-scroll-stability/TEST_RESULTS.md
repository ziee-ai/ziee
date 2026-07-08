# TEST_RESULTS — message-scroll-stability

Frontend-only diff (`src-app/ui/**`; the chat module does not exist in
`src-app/desktop/ui`), so the `ui` workspace gates apply; desktop/ui does not.

## Static gate

`npm run check (ui): PASS`

(Full chain green: tsc + biome guardrails + lint:colors + lint:settings-field +
adjacent-inline + icon-action + logical-direction + tooltip-placement +
check:kit-manifest + check:testid-registry + check:design-spec +
check:gallery-coverage + check:gallery-crawl + gallery:check-fixtures +
check:state-matrix + check:overlay-registry. Regenerated testid-registry +
state-matrix committed.)

## Unit (node:test) — 25 assertions across the 4 new/edited test files, all green

- **TEST-1**: PASS — `messageViewState.helpers.test.ts` (defaults + reset + independent key spaces)
- **TEST-2**: PASS — `inlineFileHeight.test.ts` (skeleton==body px, clamp bounds, per-viewer default)
- **TEST-3**: PASS — `scrollAnchor.utils.test.ts` (`inPlaceAnchorDelta`: pin visible, no-op above/below fold)
- **TEST-4**: PASS — `MessageViewState.store.test.ts` (message collapse round-trip + default + reset)
- **TEST-5**: PASS — `MessageViewState.store.test.ts` (file state round-trip + defaults + reset)

## E2E (Playwright, gallery `?surface=seeded-message-list-long`) — 7 passed, 0 failed

- **TEST-6**: PASS — corrections settle to ~0 after a scroll pause (top + bottom); no page errors
- **TEST-7**: PASS — inline file body is FIXED at the 400px default (caps the 180px image, doesn't hug) + row height stable across image decode
- **TEST-8**: PASS — show-more stays expanded after scroll-away-and-back (state-lift survives virtualizer remount)
- **TEST-9**: PASS — expanding a collapsed message does not jump the viewport (row top held within 6px; ITEM-7 suppression + anchor)
- **TEST-10**: PASS — keyboard resize grows the body + persists across remount
- **TEST-11**: PASS — jump-to-message lands inside the viewport + list settles afterward
- **TEST-12**: PASS — zero console/page errors across all the above interactions (afterEach guard on every test)
- **TEST-13**: PASS — pointer-drag (dispatched PointerEvents) grows the body + persists across remount

## UI evaluator gate (DoD criterion 2/3)

`gate:ui (surface seeded-message-list-long): PASS` — runtime-health reports zero
console errors / uncaught exceptions / failed requests / AA-contrast failures on
the new long-conversation surface (fixed-height body + skeleton + resize-handle
states) across themes. This is additionally proven by TEST-12 (the e2e afterEach
asserts no console/page error across every interaction on the surface).

## Notes

- E2E run via a detached session runner (`setsid`) because this Bash-tool harness
  reaps persistent server processes (exit 144); the backend-free gallery Vite
  server is managed by Playwright's own webServer inside the detached run.
  `RC=0` recorded in `msgstab-e2e.done`.
- No backend touched → no integration-test chain; no desktop/ui touched → no
  desktop workspace gate.
