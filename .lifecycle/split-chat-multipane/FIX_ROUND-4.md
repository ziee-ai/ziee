# FIX_ROUND-4 — re-audit of the FIX_ROUND-3 fixes

A blind re-audit (`split-chat-fixround4-audit`, 12 angle-reviews) of the
FIX_ROUND-3 fixes found that the per-pane File.store backup, while correct in the
store, was driven by the WRONG pane key at the extension hooks, plus a
SplitChatView remount and two spec false-greens. All fixed here; FIX_ROUND-5
re-audits these.

## Confirmed + fixed

- **HIGH — file-extension send-lifecycle hooks keyed off `focusedPaneId` across an
  async boundary** (correctness / concurrency / state-management, 3 files' worth of
  angles). `onMessageSent` runs after `await Message.send()`, and
  `onStreamError` / `afterStreamComplete` fire from `applyStreamFrame` SECONDS later
  on the OWNING pane's store — by then the user may have focused another pane, so
  `composerPaneKey(SplitView.$.focusedPaneId)` resolved to the WRONG pane:
  the sending pane's attachments were never restored/cleared (data loss) and the
  now-focused pane's live buffer was clobbered. **Fixed** by THREADING the owning
  pane id: the three `ChatExtension` hook signatures gained an `ownerPaneId?: string
  | null` param (`types.ts`); the registry dispatch passes it
  (`registry.tsx` afterStreamComplete/onMessageSent/onStreamError); each Chat.store
  dispatch site passes `get().paneId` (the owning pane's stable id, not global
  focus); and the file extension keys backup/restore/clear off `ownerPaneId`
  (DRIFT-2.13). `get().paneId` is null for single-pane → `composerPaneKey` → the
  single-pane key, so single-pane is unchanged.
- **MEDIUM — `SplitChatView` remounted every pane when crossing the `md` (768px)
  breakpoint** (state-management) — the tab branch wrapped panes in `<div key>` and
  the columns branch in `<Fragment key><div>`, so a viewport crossing changed the
  element TYPE at the same key → React unmounted+remounted every `ChatPaneProvider`
  (recreating per-pane stores, tearing down live streams — contradicting the
  "panes stay MOUNTED" guarantee). **Fixed** by unifying both modes into ONE tree
  (`<Fragment key=paneId><div key="pane">` always; the divider + tab strip toggle
  around it with their own keys), so a mode switch is a className/style/role change,
  never a remount (DRIFT-2.14).
- **MEDIUM — `registry-runtime-per-pane.spec.ts` false-green** (tests-quality) — it
  closed pane 1 of a 2-pane split, which COLLAPSES to single-pane (the survivor
  re-inits its own listener), so it could not detect the refcount-teardown
  regression it names. **Fixed:** build a 3-pane split, close the MIDDLE pane → 2
  panes remain (still split), then probe the survivor's Esc/Ctrl+K — the only setup
  that exercises the shared listener surviving a mid-split close.
- **LOW — `File.store.setBackupFiles` filtered owners with strict `=== paneKey`**
  while its paired `clearFiles` uses `composerPaneKey(owner) === paneKey`, so a
  null-owner entry could be cleared-but-not-backed-up. **Fixed:** setBackupFiles now
  uses the same `composerPaneKey(...)` resolution — backup captures EXACTLY what
  clear removes.
- **LOW — `mcp-per-pane` test name overclaimed** ("chip in pane B only, not pane
  A") vs its page-scoped assertion. **Fixed:** renamed to match (the per-pane config
  surface + applied selection); DRIFT-2.11 already documents the global chip.

**New confirmed findings:** 5
