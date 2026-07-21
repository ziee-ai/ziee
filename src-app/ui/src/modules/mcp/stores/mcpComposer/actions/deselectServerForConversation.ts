import { pendingConversationKey } from '../../approvalRouting'
import { resolveConfigKey } from '../state'
import type { McpComposerSet, McpComposerGet } from '../state'

/**
 * Per-pane (ITEM-47): deselect a server from a SPECIFIC conversation's config
 * rather than the single global-active one.
 */
export default (set: McpComposerSet, _get: McpComposerGet) => (
  conversationId: string | null,
  serverId: string,
  paneId?: string | null,
) => {
  set(state => {
    // For a new chat (no conversationId) target THIS pane's pending config
    // (ITEM-51), so removing a chip in one new-chat pane doesn't touch another.
    const key = conversationId ?? pendingConversationKey(paneId)
    const config = state.conversationConfigs.get(key)
    if (config) config.selectedServers.delete(serverId)
    if (resolveConfigKey(state, state.currentConversationId) === key) {
      state.selectedServers.delete(serverId)
    }
  })
}
