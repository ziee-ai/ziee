# PLAN — tool-artifact-grouping (follow-up #3: make the approval scroll actually work)

#134's approval-scroll is a no-op in the real app (native `scrollIntoView` on a
virtualized row inside the OverlayScrollbars viewport doesn't move it) and its e2e was
a false-green (spied the call, one short on-screen message). Fix it with the app's own
virtualization-aware scroll, bypassing the `isAtBottom` gate, and prove it with a real
below-the-fold e2e. Frontend (React/TS) only. Scope: ONLY the approval-scroll + its
test — do NOT touch the single-tool artifact wrapping.

## Items

- **ITEM-1**: In `ConversationPage.tsx`, add a reactive `const { toolCalls } = Stores.McpComposer` subscription and an effect that, when a NEW `pending_approval` `tool_use_id` appears in `toolCalls`, calls `messageListRef.current?.scrollToBottom()` (the app's virtualization-aware scroll) UNCONDITIONALLY — bypassing the `isAtBottomRef` gate that currently suppresses following an off-bottom approval. Fire once per `tool_use_id` (dedupe via `useRef(new Set<string>())`); gate only on the same conversation-match guard the auto-follow uses (`conversation?.id === conversationId && initialScrollConvIdRef.current === conversationId`), never on `isAtBottom`.
- **ITEM-2**: In `ToolCallPendingApprovalContent.tsx`, remove the now-dead scroll code from #134: the module-level `scrolledApprovals` Set, the `containerRef` + the `useEffect` `scrollIntoView`, the `ref` on the outer div, and the `useEffect`/`useRef` imports if unused. The component is otherwise unchanged.
- **ITEM-3**: Rewrite the scroll test in `tests/e2e/07-mcp/tool-group-single-artifact.spec.ts` (replace the `scrollIntoView`-spy test) to a real below-the-fold reproduction: overflow the list, scroll the message-list viewport to top (assert `chat-jump-to-latest-btn` visible ⇒ `isAtBottom===false`), stream a tail `mcpApprovalRequired`, and assert `toBeInViewport` on `tool-approval-<id>` — which fails without the fix and passes with it. Keep the #134 single-tool wrapping tests unchanged.

## Files to touch

- EDIT `src-app/ui/src/modules/chat/pages/ConversationPage.tsx`
- EDIT `src-app/ui/src/modules/mcp/chat-extension/components/ToolCallPendingApprovalContent.tsx`
- EDIT `src-app/ui/tests/e2e/07-mcp/tool-group-single-artifact.spec.ts`

## Patterns to follow

- **Reactive McpComposer read** mirrors `McpToolUseRenderer` / `McpToolGroupCard` in `mcp/chat-extension/extension.tsx` (`const { toolCalls } = Stores.McpComposer`; `McpToolCall` from `@/modules/mcp/stores/McpComposer.store`).
- **scrollToBottom** is the existing `MessageListHandle.scrollToBottom()` (`MessageList.tsx:385`), already used by ConversationPage's initial-load effect (`ConversationPage.tsx:272`). The effect mirrors ConversationPage's existing auto-follow effect (`:288-298`) but drops the `isAtBottomRef` gate and adds per-id dedupe.
- **E2E** mirrors `tests/e2e/chat/jump-to-latest.spec.ts` (overflow + scroll-to-top + `chat-jump-to-latest-btn` as the not-at-bottom proxy) combined with the `07-mcp/tool-group-auto-open.spec.ts` approval-stream pattern (`mockChatTokenStream` multi-script + `mcpApprovalRequiredEvent` + `mockGetMessages`). Scroll the `[data-overlayscrollbars-viewport]` (message-list viewport); approval testid `tool-approval-<tool_use_id>`.
