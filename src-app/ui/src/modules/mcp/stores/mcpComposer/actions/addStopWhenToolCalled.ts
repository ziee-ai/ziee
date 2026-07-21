import { resolveConfigKey } from '../state'
import type { McpComposerSet, McpComposerGet } from '../state'
import type { ToolIdentifier } from '@/api-client/types'

/**
 * Add a tool to stop_when_tools_called.
 */
export default (set: McpComposerSet, _get: McpComposerGet) => (
  conversationId: string | null,
  tool: ToolIdentifier,
) => {
  set(state => {
    const configKey = resolveConfigKey(state, conversationId)
    let config = state.conversationConfigs.get(configKey)

    // Create config if it doesn't exist (for both new and existing conversations)
    if (!config) {
      config = {
        selectedServers: new Map(),
        disabledServers: [],
        approvalMode: 'manual_approve',
        autoApprovedTools: [],
        loopSettings: {},
      }
      state.conversationConfigs.set(configKey, config)
    }

    const current = config.loopSettings?.stop_when_tools_called || []
    // Avoid duplicates
    if (!current.some(t => t.server_id === tool.server_id && t.tool_name === tool.tool_name)) {
      config.loopSettings = {
        ...config.loopSettings,
        stop_when_tools_called: [...current, tool],
      }
    }
  })
}
