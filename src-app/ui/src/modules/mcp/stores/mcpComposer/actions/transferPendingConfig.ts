import { pendingConversationKey } from '../../approvalRouting'
import type { McpComposerSet, McpComposerGet } from '../state'

/**
 * Transfer pending config to a real conversation ID.
 */
export default (set: McpComposerSet, _get: McpComposerGet) => (
  conversationId: string,
  paneId?: string | null,
) => {
  set(state => {
    // Move THIS pane's pending config to the freshly-minted conversation id
    // (ITEM-51): the sending pane's own pending key, not the shared one.
    const pendingKey = pendingConversationKey(paneId)
    const pendingConfig = state.conversationConfigs.get(pendingKey)
    if (pendingConfig) {
      // Copy pending config to new conversation
      state.conversationConfigs.set(conversationId, {
        ...pendingConfig,
        selectedServers: new Map(pendingConfig.selectedServers),
      })
      // Clear this pane's pending config
      state.conversationConfigs.delete(pendingKey)
      console.log('[MCP Store] Transferred pending config to conversation:', conversationId)
    }
  })
}
