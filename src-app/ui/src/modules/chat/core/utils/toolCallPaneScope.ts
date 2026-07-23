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
/**
 * The set of ids a pending approval can correlate to for THIS pane: every
 * message id PLUS every tool_use content id found in those messages. Pass the
 * result as the `paneScope` to {@link pendingApprovalIdsInPane} so an approval
 * whose `message_id` is unset (it led its turn, before its assistant message
 * existed) still scopes correctly via its tool_use content id.
 */
export function paneApprovalScope(
  messages: Iterable<
    [string, { contents?: Array<{ content_type?: string; content?: unknown }> } | undefined]
  >,
): Set<string> {
  const scope = new Set<string>()
  for (const [msgId, msg] of messages) {
    scope.add(msgId)
    for (const c of msg?.contents ?? []) {
      if (c.content_type === 'tool_use') {
        const tid = (c.content as { id?: string } | undefined)?.id
        if (tid) scope.add(tid)
      }
    }
  }
  return scope
}

export function pendingApprovalIdsInPane(
  toolCalls: Iterable<[string, ToolCallLike]>,
  paneScope: { has(id: string): boolean },
): string[] {
  const ids: string[] = []
  for (const [id, call] of toolCalls) {
    // Correlate the approval to THIS pane by EITHER its carrying message id (when
    // the store stamped one) OR its `tool_use_id` (the map key) appearing among
    // the pane's rendered tool_use contents. The tool_use path is the reliable
    // one: an approval that LEADS a turn is registered BEFORE its assistant
    // message exists, so its `message_id` is unset (null) — but the tool_use
    // content id is always present once the message renders. Callers seed
    // `paneScope` with BOTH this pane's message ids AND the tool_use ids in its
    // messages (see `paneApprovalScope`).
    if (
      call.status === 'pending_approval' &&
      (toolCallInPane(call, paneScope) || paneScope.has(id))
    ) {
      ids.push(id)
    }
  }
  return ids
}
