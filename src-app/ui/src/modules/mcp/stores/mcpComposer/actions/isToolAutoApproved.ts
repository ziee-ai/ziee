import { resolveConfigKey } from '../state'
import type { McpComposerGet } from '../state'

/**
 * Check if a tool is auto-approved for current conversation (or pending)
 * — synchronous.
 */
export default (_set: unknown, get: McpComposerGet): (serverId: string, toolName: string) => boolean => {
  return (serverId: string, toolName: string): boolean => {
    const state = get()
    const configKey = resolveConfigKey(state, state.currentConversationId)

    const config = state.conversationConfigs.get(configKey)
    if (!config || !config.autoApprovedTools) return false

    // Find server entry and check if tool is in its tools array
    const serverEntry = config.autoApprovedTools.find(s => s.server_id === serverId)
    return serverEntry ? serverEntry.tools.includes(toolName) : false
  }
}
