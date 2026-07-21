import { resolveConfigKey } from '../state'
import type { McpComposerSet, McpComposerGet } from '../state'

/**
 * Deselect a server.
 */
export default (set: McpComposerSet, _get: McpComposerGet) => (serverId: string) => {
  set(state => {
    state.selectedServers.delete(serverId)

    // Update conversation config (or pending config)
    const configKey = resolveConfigKey(state, state.currentConversationId)
    const config = state.conversationConfigs.get(configKey)
    if (config) {
      config.selectedServers.delete(serverId)
    }
  })
  console.log('[MCP Store] Deselected server:', serverId)
}
