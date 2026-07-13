# FIX_ROUND-22 — split-chat-multipane (ITEM-70 header-fix convergence)

A fresh BLIND diff-only reviewer re-checked the header-fix delta (unified
`onPaneArea*` handlers attached to BOTH the pane header and the chat column) for
any NEW defect the fix introduces. Traced the full DOM structure + helpers.

## Verdict: the fix converges cleanly — no new defect.

- **No double-handling** — the header (direct child of the outer `flex flex-col`)
  and the chat column (grandchild via the main-area div) are in DISJOINT subtrees,
  never ancestor/descendant, so a drop (+ its bubbling ancestors) reaches EXACTLY
  ONE `onPaneAreaDrop`. A `file` drag hits the guard (no preventDefault, no state
  write) → no cross-fire.
- **Grip self-reorder is a clean no-op** — dropping a pane's own grip on its header
  (or, newly, its column) → `reorderIndices(from===to)` returns null → no reorder,
  no preventDefault.
- **Zone math consistent** — the header is a conversation drop target ONLY when
  `pane` is truthy (split); there the right panel renders as an `absolute` overlay
  (out of flow), so the column keeps the pane's FULL width == header width. Same
  `zoneForX` fraction on both. (The in-flow side panel that would narrow the column
  exists only on the single-pane route, where the header is NOT a drop target.)
- **No stuck state** — `onPaneAreaDragLeave` clears BOTH `dropZone` + `paneDropActive`
  behind the `contains(relatedTarget)` real-exit guard (removes the old unguarded
  header-child flicker); a drag carries one kind, so the two flags never co-set.
- **Rules of Hooks clean** — no hooks added/moved; the overlay `atCap` still reads
  the `.$` snapshot.
- **preventDefault correct** — a conversation over the header calls preventDefault
  (when a conversation is present), so the real-browser drop fires.

The intended behavior change (a pane-reorder drag over the column now lights the
header ring + accepts the drop = "whole pane area is the reorder target") is
deliberate, not a bug. Original bug fixed; merge introduces no new defect.

**New confirmed findings:** 0
