/**
 * Pure helpers for computing the navigator anchor message in each fork group.
 *
 * Extracted as pure functions so they can be unit-tested without mocking
 * Zustand stores or dynamic imports.
 */

export type ForkLevel = 'user' | 'assistant'

/** Minimal shape needed for anchor calculations */
export interface AnchorMessage {
  id: string
  role: string
  created_at: string
}

/**
 * Compute the anchor message for the PARENT branch in a fork group.
 *
 * All children in a group share the same forkLevel (the group key encodes it),
 * so the level is passed directly rather than derived from child inspection.
 *
 * - 'assistant' level (regenerate): both parent and child share the same user
 *   message → anchor at the FIRST ASSISTANT message after the fork point.
 *
 * - 'user' level (edit): the forked user message is the divergence point
 *   → anchor at forkMsgId itself.
 */
export function computeParentAnchor(
  forkMsgId: string,
  forkLevel: ForkLevel,
  messages: AnchorMessage[],
  messageIds: Set<string>,
): string | null {
  if (forkLevel === 'assistant') {
    const forkMsgIndex = messages.findIndex(m => m.id === forkMsgId)
    const firstAssistantAfterFork =
      forkMsgIndex !== -1
        ? messages.slice(forkMsgIndex + 1).find(m => m.role === 'assistant')
        : undefined
    return firstAssistantAfterFork?.id ?? null
  }

  // Edit flow: anchor at the fork origin message
  return messageIds.has(forkMsgId) ? forkMsgId : null
}

/**
 * Compute the anchor message for a CHILD branch in a fork group.
 *
 * The backend clones messages with created_at < fork-message's timestamp,
 * so messages with created_at >= branch.created_at are "new" to this branch.
 *
 * - 'assistant' level fork (regenerate flow):
 *   The shared messages already include the original user message.
 *   New messages are: new_user_msg → new_assistant_msg.
 *   Anchor at the first new ASSISTANT message to match the parent's anchor.
 *
 * - 'user' level fork (edit flow, default):
 *   The forked user message is excluded from the clone.
 *   First new message is the edited user message → anchor there.
 */
export function computeChildAnchor(
  activeBranchId: string,
  branchCreatedAt: string,
  messages: AnchorMessage[],
  branchForkLevels: Map<string, ForkLevel>,
): string | null {
  const forkLevel = branchForkLevels.get(activeBranchId) ?? 'user'
  const branchCreatedAtMs = new Date(branchCreatedAt).getTime()

  const firstNewMsgIdx = messages.findIndex(
    m => new Date(m.created_at).getTime() >= branchCreatedAtMs,
  )
  if (firstNewMsgIdx === -1) return null

  if (forkLevel === 'assistant') {
    // Anchor at the first new assistant message
    const firstNewAssistant = messages
      .slice(firstNewMsgIdx)
      .find(m => m.role === 'assistant')
    return firstNewAssistant?.id ?? messages[firstNewMsgIdx].id
  }

  // Edit flow: anchor at the first new message (edited user message)
  return messages[firstNewMsgIdx].id
}
