# DRIFT-1 — implementation vs plan (follow-up #3)

- **DRIFT-1.1** — verdict: none — ITEM-1 (ConversationPage effect: watch `toolCalls` →
  `messageListRef.scrollToBottom()`, bypass `isAtBottom`, dedupe per id, conversation-gated)
  and ITEM-2 (remove the dead `scrollIntoView` + `scrolledApprovals` Set from
  `ToolCallPendingApprovalContent`) implemented exactly as planned.

- **DRIFT-1.2** — verdict: impl-wins — the e2e reproduction mechanism (ITEM-3) evolved
  during implementation. The plan sketched a seeded-conversation (`/chat/:id`) + scroll +
  send flow; in practice a seeded conversation's composer left the **Send button disabled**
  (no model attached via `selectModelInDropdown` on an existing conversation), so no turn
  fired. Switched to the proven `goToNewChatPage` send path with a **two-turn** flow: turn 1
  streams a long answer to overflow the virtualized list, then scroll the message-list
  viewport to the top (`isAtBottom===false`, asserted via `chat-jump-to-latest-btn` visible),
  then turn 2 streams the tail approval, and assert `toBeInViewport`. The plan's INTENT
  (assert the EFFECT in a below-the-fold scenario) is unchanged, so no PLAN amendment — this
  is an implementation detail of the same test. Also dropped an over-strict `toBeHidden`
  sanity check after turn 1 (new-chat streaming does not reliably leave the view at the
  bottom, and it isn't needed — the scroll-to-top forces not-at-bottom).

- **DRIFT-1.3** — verdict: resolved — **negative check performed** (the whole point of this
  follow-up, after #134's false-green): temporarily disabled `scrollToBottom()`, rebuilt, and
  confirmed the e2e **FAILS on `toBeInViewport`** (approval renders but stays below the fold);
  restored the fix → passes. The test genuinely catches the regression.

**Unresolved drifts:** 0
