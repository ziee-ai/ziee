# DRIFT-7 — split-chat-multipane (ITEM-51: per-pane pending KB + MCP)

Implementation-vs-plan reconciliation for ITEM-51 (round-13 follow-up FB-11,
human-directed): key the PRE-MINT new-chat PENDING buffer per pane in BOTH the KB
and MCP composers, symmetrically. Rounds 1–5 + round-13 already converged.

- **DRIFT-7.1** — verdict: none — the store/key change shipped exactly as DEC-67
  specifies: `pendingKbKey(paneId)` (in the pure `kbSelectionKey.ts`) +
  `pendingConversationKey(paneId)` (in the pure `approvalRouting.ts`, alongside its
  twin `PENDING_CONVERSATION_KEY` so it's node-testable), each `${base}:${paneId}`
  for a real pane and the bare base key when paneId is null/empty (single-pane
  byte-identical). paneId threaded as an OPTIONAL trailing param through every
  pending-touching action; MCP's `currentPaneId` is consulted ONLY in
  `resolveConfigKey`'s pending branch. Read/write sites resolve paneId via the
  proven `useChatPaneOrNull()?.paneId`. TEST-76/77 unit + TEST-78 e2e all green.

- **DRIFT-7.2** — verdict: resolved — the TEST-78 e2e setup drifted from the naive
  plan sketch (which imagined "split from a new chat"). Reality: the `chat-split-btn`
  lives in a ConversationPane header, which the bare `/chat` new-chat page does NOT
  render, so a split cannot be initiated from a new chat. Resolved by anchoring the
  split on a saved conversation (pane 0) and adding TWO further panes that each start
  their OWN new chat via the picker's `pane-start-new-chat` — so panes 1 AND 2 are
  both NEW (pre-mint) chats (MAX_PANES=3 accommodates this), which is the exact
  two-new-chat case FB-11 describes, and pane 0 is a mere anchor. This is a stronger
  reproduction than a new-vs-saved pair (a saved second pane reads its conversation
  id, never the pending key, so it could not exhibit the leak at all).

- **DRIFT-7.3** — verdict: resolved — the e2e legs had to be ordered MCP-first. The
  KB attach path leaves its `+` dropdown / submenu popover open (it's the terminal
  interaction in the sibling TEST-69), so doing KB first then re-opening the `+`
  menu for MCP toggled the still-open dropdown shut and the MCP menu item never
  appeared. The MCP config-modal path calls the dropdown's `close()` on open and the
  modal closes cleanly, leaving a fresh state — so running MCP first then KB (KB
  terminal) is reliable. Test-mechanics only; no product change.

**Unresolved drifts:** 0
