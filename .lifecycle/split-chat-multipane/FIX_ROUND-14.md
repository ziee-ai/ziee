# FIX_ROUND-14 — split-chat-multipane (ITEM-51 blind audit)

Blind adversarial review (1 fresh diff-only reviewer) of the ITEM-51 delta
(per-pane PENDING KB + MCP), tracing every write/read site, the transfer-on-send
flow, the `currentPaneId` lifecycle, and single-pane parity.

- **KB** — internally consistent. Its send path INJECTS nothing (search_knowledge
  resolves the conversation's attached KBs server-side); the composer only
  persists via the pane-aware `transferPending`. No finding.

## Confirmed + fixed

- **HIGH — MCP send read left bare-keyed** (McpComposer.store.ts
  `getSelectedServersConfigFor`). ITEM-51 made the pending WRITE per-pane but the
  primary SEND read still keyed `PENDING_CONVERSATION_KEY`, so a new-chat split
  pane's first message dropped its `mcp_config` (miss + empty same-conv fallback)
  or, when `currentConversationId` was null, leaked the global `selectedServers`
  (whichever pane last opened its modal). Fixed: the read takes `paneId` and keys
  the pending case by `pendingConversationKey(paneId)`; the global-projection
  fallback is restricted to a REAL `conversationId === currentConversationId`
  (never the pending/null case) — a pending pane with no stored config sends `[]`,
  never another pane's selection. `composeRequestFields` passes `ctx.paneId`.
- **MEDIUM — new-chat DEFAULT seeding left bare-keyed** (McpInitializer +
  applyUserDefaultsToPending + the no-defaults selectServer loop). Found while
  fixing the HIGH: the per-pane McpInitializer wrote a new pane's default MCP
  selection to the bare pending key, so a new SPLIT pane's defaults would neither
  show nor send (ITEM-51-introduced UX regression). Fixed: `applyUserDefaultsToPending`
  + `selectServer` accept an optional `paneId` and write the pane's own pending
  config (creating it if absent); McpInitializer resolves its pane via
  `useChatPaneOrNull`, gates on the PANE's own conversation (not the global
  `currentConversationId` pointer), and threads `paneId`.
- **LOW — approval-mode/auto-approve-tool create-vs-read key divergence**
  (setApprovalMode + toggleAutoApprovedTool). Both created the fallback pending
  config under the bare key while reading `configKey` from the pane-aware
  `resolveConfigKey`. Fixed: both create under `configKey`.

All fixes preserve single-pane parity (null paneId → the bare key, byte-identical).
Re-verified: tsc + npm run check both workspaces; TEST-76/77 unit + TEST-78 e2e
(per-pane pending display) + TEST-71 (MCP saved-conv, regression) green.

**New confirmed findings:** 0
