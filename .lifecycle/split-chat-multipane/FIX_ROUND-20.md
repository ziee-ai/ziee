# FIX_ROUND-20 — split-chat-multipane (audit round convergence re-audit)

A fresh BLIND diff-only reviewer re-checked the audit-round fix delta (ITEM-59..69 —
the per-pane routing fixes + `PaneDraftKeys`/`ownerChatState`) for any NEW defect the
fixes introduce. Traced against the real code (ChatPaneContext, chatBridge,
Chat.store invocation sites, SplitView defaults, SkillDetailDrawer render sites, the
text-extension hooks).

## Verdict: the fixes converge cleanly — no new defect.

- **React hooks rules — clean.** `useChatPaneOrNull()` is called unconditionally at the
  top of every touched component (ConversationFindBar, EditingMessageBanner,
  CanvasSelectionPopover, TitleEditor) before any early return.
- **Deps / stale closures — sound.** `ConversationFindBar.activateMatch` deps include the
  stable `chat`; the Cmd-F `[]`-effect reads the stable `pane.paneId` + the live
  `SplitView.$.focusedPaneId` snapshot (adding `pane` to an already-`[]`-deps effect is
  no new lint posture).
- **`PaneDraftKeys`/`ownerChatState` — correct.** `paneKeyOf(null)==''`, `take` one-shot;
  `ownerChatState` resolves the pane's own `TextStore` in split and falls back to
  `Chat.store.getState()` in single-pane (byte-identical to the old bridge read). The
  double-cast is sound (both fields are present at runtime).
- **Single-pane regression — preserved.** `focusedPaneId` + `paneId` both default to
  `null`, so single-pane does `set(null)→''` / `take(null)→''`; draft clear/restore is
  unchanged. The skill drawer opens in single-pane (`openConversationId === conversationId`).

One edge was explicitly considered and REJECTED as a new defect: a PROGRAMMATIC send on a
non-focused pane (e.g. an MCP approval resume while another pane is focused) keys the
capture under `focusedPaneId` but takes under the sending `get().paneId`, so they miss
and no draft is cleared — but this is NOT a regression (the old module-global cleared the
WRONG focused pane's draft; "do nothing" is no worse, and an approval resume carries no
user-authored composer draft). Pre-existing limitation of `beforeSendMessage` not
receiving a paneId, scoped by the fix comment; recorded here, not a new finding.

**New confirmed findings:** 0
