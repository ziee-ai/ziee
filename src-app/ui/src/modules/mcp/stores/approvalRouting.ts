import type { ToolApprovalDecision } from '@/api-client/types'

/**
 * Per-conversation MCP tool-approval routing (ITEM-33) — the pure logic behind
 * the wrong-pane approval fix, extracted so it is unit-testable WITHOUT importing
 * the enum-laden `McpComposer.store` (node's strip-only type mode rejects enums).
 *
 * Approvals are filed under the ORIGINATING conversation's key, so approving a
 * tool in one split pane can never be picked up by another pane's send: the send
 * path reads only its own conversation's decisions.
 */

export const PENDING_CONVERSATION_KEY = '__pending__'

/** The key an approval belongs to — its conversation, or pending for a new chat. */
export const approvalKeyOf = (
  conversationId: string | null | undefined,
): string => conversationId ?? PENDING_CONVERSATION_KEY

/**
 * Per-PANE pending (new-chat, pre-mint) config key: `__pending__:<paneId>`, or the
 * bare `PENDING_CONVERSATION_KEY` when there is no pane (single-pane), so two split
 * panes each composing a NEW chat don't share one pending config (ITEM-51 —
 * symmetric with KB's `pendingKbKey`). Lives here (pure) alongside
 * `PENDING_CONVERSATION_KEY` so it's unit-testable without the ApiClient-laden store.
 */
export const pendingConversationKey = (paneId?: string | null): string =>
  paneId ? `${PENDING_CONVERSATION_KEY}:${paneId}` : PENDING_CONVERSATION_KEY

/** Append a decision to a conversation's list (returns a NEW map — immer-safe). */
export function addApprovalDecisionTo(
  map: Map<string, ToolApprovalDecision[]>,
  key: string,
  decision: ToolApprovalDecision,
): Map<string, ToolApprovalDecision[]> {
  const next = new Map(map)
  next.set(key, [...(map.get(key) ?? []), decision])
  return next
}

/** The decisions filed for one conversation (empty array when none). */
export function getApprovalDecisionsFrom(
  map: Map<string, ToolApprovalDecision[]>,
  key: string,
): ToolApprovalDecision[] {
  return map.get(key) ?? []
}

/** Drop one conversation's decisions (returns a NEW map — immer-safe). */
export function clearApprovalDecisionsIn(
  map: Map<string, ToolApprovalDecision[]>,
  key: string,
): Map<string, ToolApprovalDecision[]> {
  const next = new Map(map)
  next.delete(key)
  return next
}
