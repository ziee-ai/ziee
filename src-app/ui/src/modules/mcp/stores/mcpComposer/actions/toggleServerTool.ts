import { resolveConfigKey } from '../state'
import type { McpComposerSet, McpComposerGet } from '../state'

/**
 * Toggle a specific tool for a server.
 */
export default (set: McpComposerSet, _get: McpComposerGet) => (
  serverId: string,
  toolName: string,
) => {
  set(state => {
    const selection = state.selectedServers.get(serverId)
    if (!selection) return

    const toolIndex = selection.tools.indexOf(toolName)
    let newTools: string[]

    if (toolIndex >= 0) {
      // Tool is selected, remove it
      newTools = selection.tools.filter((_, index) => index !== toolIndex)
    } else {
      // Tool not selected, add it
      newTools = [...selection.tools, toolName]
    }

    const newSelection = {
      server_id: serverId,
      tools: newTools,
    }

    state.selectedServers.set(serverId, newSelection)

    // Update conversation config (or pending config)
    const configKey = resolveConfigKey(state, state.currentConversationId)
    const config = state.conversationConfigs.get(configKey)
    if (config) {
      config.selectedServers.set(serverId, newSelection)
    }
  })
}
