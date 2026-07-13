# DRIFT-12 — split-chat-multipane (ITEM-70: per-pane edge-directional drop in splits)

Reconciliation for ITEM-70 (unify the split-view conversation drop onto the single-pane
edge-directional model), built after the human live-tested and asked how drag-drop
should behave with a split already open.

- **DRIFT-12.1** — verdict: impl-wins→fixed — the ORIGINAL DEC-25 specified "drop on a
  pane's **edge drop-zone** = split (Split-left / Replace / Split-right)". ITEM-31
  UNDER-DELIVERED this as a discrete model (drop-on-header = replace, drop-on-seam = new
  pane), so the single-pane edge-directional gesture (ITEM-57) didn't carry into an
  existing split — an inconsistency the human caught by running. ITEM-70 now delivers the
  original DEC-25 edge-drop-zone intent for the split case too: per-pane L/C/R (left =
  insert-before, right = insert-after, center = replace), with the hint overlay, at cap
  falling back to replace. The header/seam conversation handlers are retired (edges cover
  them); grip→header reorder stays. RUN by TEST-105/107 (unit) + TEST-106 (e2e).

- **DRIFT-12.2** — verdict: resolved — a Rules-of-Hooks regression I introduced in the
  ITEM-70 overlay (a REACTIVE `Stores.SplitView.panes` read inside the overlay `.map()`
  and inside the `{dropZone && …}` conditional → a hook called a varying number of times
  → "Rendered more hooks than during the previous render"). The unit tests could NOT see
  it (pure functions); only a LIVE render exposed it — the human caught it running the
  app. Fixed to the non-subscribing `.$` snapshot read (a plain value, not a hook), and
  the drag-to-split e2e (which actually renders the overlay mid-drag) now locks it in.
  Lesson recorded FB-17: an overlay/derived value that reads a store inside a loop or a
  conditional must use `.$` (snapshot), never the reactive proxy.

- **DRIFT-12.3** — verdict: none — the store `openPane` gained a `beforePaneId` option
  (symmetric with the existing `afterPaneId`), unit-tested; `planSplitPaneDrop` is a pure
  sibling of `planSinglePaneDrop`; the single-pane `onColumnDrop`/overlay generalize by
  dispatching on `pane` (single vs split). No new migration/OpenAPI; desktop shares the
  same `ui/src`.

**Unresolved drifts:** 0
