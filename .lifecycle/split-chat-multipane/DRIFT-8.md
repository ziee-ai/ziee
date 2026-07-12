# DRIFT-8 — split-chat-multipane (round 6: desktop pop-out UX)

Implementation-vs-plan reconciliation for round-6 (FB-12 → ITEM-52/53/54). Built
test-first per the standing working-mode rules.

- **DRIFT-8.1** — verdict: none — ITEM-52/53/54 shipped as planned: a layout-less
  `/chat-window/:id` route (chat-only), a `focusPopoutWindowIfOpen` web/desktop seam
  at the top of the sole open path, and a `POPOUT_CLOSED_EVENT` snap-back with a pure
  decision + a Tauri wiring seam. Every item RUNS its behavior (TEST-79 real render
  DOM; TEST-80/83 mocked-Tauri control flow; TEST-81/82/84 pure).

- **DRIFT-8.2** — verdict: resolved — the blind-audit HIGH: snap-back mutated the
  store but never navigated, so on a non-chat route the pane never rendered
  (`SplitChatView` lives only inside `ConversationPage`). Reconciled by extracting
  `snapBackAsNewPane` (store-open THEN navigate) + TEST-84 — the plan under-specified
  "open as a pane" as a store call when it also needs the navigate the sibling hook
  already does. Impl now matches the sibling.

- **DRIFT-8.3** — verdict: resolved — `PopoutConversationPage` is a new page
  component, so the gallery-surface scanner added it to `GallerySurface`; the
  hand-maintained `GALLERY_COVERAGE` in `coverage.ts` then failed `satisfies
  Record<GallerySurface, Coverage>` (missing key). Added a `via` coverage entry
  (covered by the pop-out e2e). Also regenerated the mechanical
  `galleryCoverage.generated.ts` / `OVERRIDE_MANIFEST.md` (2 new `.desktop` seams) /
  `stateMatrix.generated.ts`. Reconciled; `npm run check` green both workspaces.

- **DRIFT-8.4** — verdict: none — the ITEM-54 Tauri cross-OS-window event DELIVERY
  is a platform behavior not runnable on this Linux box; flagged for desktop-host
  verification (FIX_ROUND-15 / PLAN_AUDIT ITEM-54). NOT claimed working; the owned
  logic (decision/handler/emit/listen control flow) is unit-run.

**Unresolved drifts:** 0
