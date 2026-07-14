/**
 * Per-pane scoping of the process-global `McpComposer.toolCalls` map (ITEM-48).
 *
 * The tool-call map is conversation-agnostic and never cleared across
 * conversations, so a pending-approval tool in one split pane would otherwise
 * yank EVERY pane's message list to its bottom. A tool-call belongs to THIS pane
 * iff the streaming message that carries it (`message_id`) is one of this pane's
 * own loaded messages — the same `message_id` correlation ITEM-33 already uses to
 * keep two panes' same-server calls from cross-bleeding. The scroll-to-approval
 * seed + scroll loops filter through this so only the ORIGINATING pane scrolls.
 *
 * Pure — takes a `has(id)` predicate for the pane's message ids, so it is
 * node-testable without importing the Chat/McpComposer stores.
 */

type ToolCallLike = { status?: string; message_id?: string | null }

/** True iff `call`'s carrying message is one of this pane's own messages. */
export function toolCallInPane(
  call: ToolCallLike,
  paneMessageIds: { has(id: string): boolean },
): boolean {
  return call.message_id != null && paneMessageIds.has(call.message_id)
}

/**
 * The ids of `pending_approval` tool-calls that belong to THIS pane (their
 * carrying message is in `paneMessageIds`) — the seed + scroll loops iterate
 * only these, so a pending approval in another pane's conversation is ignored.
 */
export function pendingApprovalIdsInPane(
  toolCalls: Iterable<[string, ToolCallLike]>,
  paneMessageIds: { has(id: string): boolean },
): string[] {
  const ids: string[] = []
  for (const [id, call] of toolCalls) {
    if (call.status === 'pending_approval' && toolCallInPane(call, paneMessageIds)) {
      ids.push(id)
    }
  }
  return ids
}
