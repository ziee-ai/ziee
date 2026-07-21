import { resolveConfigKey } from '../state'
import type { McpComposerSet, McpComposerGet } from '../state'


/**
 * Toggle auto-approved status for a tool.
 */
export default (set: McpComposerSet, _get: McpComposerGet) => (
  conversationId: string | null,
  serverId: string,
  toolName: string,
) => {
  set(state => {
    const configKey = resolveConfigKey(state, conversationId)
    let config = state.conversationConfigs.get(configKey)

    // Create pending config if it doesn't exist (for new conversations)
    if (!config && !conversationId) {
      config = {
        selectedServers: new Map(),
        disabledServers: [],
        approvalMode: 'manual_approve',
        autoApprovedTools: [],
      }
      state.conversationConfigs.set(configKey, config)
    }

    if (!config) return

    const autoApproved = config.autoApprovedTools || []

    // Find existing server entry
    const serverIndex = autoApproved.findIndex(s => s.server_id === serverId)

    if (serverIndex >= 0) {
      // Server exists, toggle tool in its tools array
      const server = autoApproved[serverIndex]
      const toolIndex = server.tools.indexOf(toolName)

      if (toolIndex >= 0) {
        // Tool exists, remove it
        const newTools = server.tools.filter((_, i) => i !== toolIndex)
        if (newTools.length === 0) {
          // No more tools for this server, remove server entry
          config.autoApprovedTools = autoApproved.filter((_, i) => i !== serverIndex)
        } else {
          // Update server with remaining tools
          config.autoApprovedTools = autoApproved.map((s, i) =>
            i === serverIndex ? { ...s, tools: newTools } : s,
          )
        }
      } else {
        // Tool doesn't exist, add it
        config.autoApprovedTools = autoApproved.map((s, i) =>
          i === serverIndex ? { ...s, tools: [...s.tools, toolName] } : s,
        )
      }
    } else {
      // Server doesn't exist, create new entry
      config.autoApprovedTools = [...autoApproved, { server_id: serverId, tools: [toolName] }]
    }
  })
}
