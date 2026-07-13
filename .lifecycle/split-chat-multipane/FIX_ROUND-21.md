# FIX_ROUND-21 — split-chat-multipane (ITEM-70 blind audit)

Blind adversarial review (1 fresh diff-only reviewer) of the ITEM-70 delta
(per-pane edge-directional drop in existing splits). Verified correct: the
Rules-of-Hooks fix (`.$` snapshot, no other reactive-in-loop reads),
`planSplitPaneDrop` math, `openPane({beforePaneId})` splice, dedup, cap fallback,
no double-handling in the original layout, and clean import hygiene.

## Fixed (confirmed)

- **MEDIUM (CONFIRMED)** — a conversation dropped on a split pane's HEADER was
  SILENTLY SWALLOWED. The header (`chat-pane-header`, an `h-11` strip) is a SIBLING
  of the chat column (`data-pane-drop-column`), not an ancestor, so a header drop
  can't bubble to the column's handler; and the narrowed header handler
  preventDefault'd only pane drags, so on a REAL browser the drop event never fired
  (no feedback). My code comment wrongly claimed "falls through to the column via
  bubbling"; Playwright's synthetic `dispatchEvent` (fires regardless of
  preventDefault) masked it in the e2e. **Fix:** merged the header + column handlers
  into unified `onPaneArea{DragOver,DragLeave,Drop}` that dispatch by drag kind
  (conversation → edge-directional; pane → reorder) and attached them to BOTH the
  header and the column — the whole pane is now one live drop target that
  preventDefaults dragover for conversations. Added a header-drop e2e leg
  (TEST-106) that distinguishes fixed (replace) from pre-fix (no-op, header ignored
  conversations). e2e 4/4 green.

## Recorded, not fixed (LOW / out of scope)

- **LOW (CONFIRMED)** — dropping a conversation onto an EMPTY split pane is a no-op.
  MOOT for this diff: an empty pane renders `ConversationPickerPane`
  (`ConversationPage` returns it at the `pane && !conversationId && !conversation`
  branch), NOT the drop column — it's filled by CLICKING the picker. Drop-to-fill
  the picker is a possible future enhancement, out of ITEM-70's scope (dropping onto
  panes that HOLD conversations).
- **doc nit** — `paneDnd.ts`'s module comment (drop zones "live on the pane HEADER +
  the inter-pane SEAM") was stale after ITEM-70; updated to "the whole pane area
  (header + column), unified handler".

**New confirmed findings:** 1
