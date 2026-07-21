import { resolveConfigKey } from '../state'
import type { McpComposerSet, McpComposerGet } from '../state'
import type { PerToolLimit } from '@/api-client/types'

/**
 * Add a per-tool iteration limit.
 */
export default (set: McpComposerSet, _get: McpComposerGet) => (
  conversationId: string | null,
  limit: PerToolLimit,
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

    const current = config.loopSettings?.per_tool_max_iteration || []
    // Avoid duplicates - update existing if found
    const existingIndex = current.findIndex(
      t => t.server_id === limit.server_id && t.tool_name === limit.tool_name
    )
    if (existingIndex >= 0) {
      // Update existing
      const updated = [...current]
      updated[existingIndex] = limit
      config.loopSettings = {
        ...config.loopSettings,
        per_tool_max_iteration: updated,
      }
    } else {
      // Add new
      config.loopSettings = {
        ...config.loopSettings,
        per_tool_max_iteration: [...current, limit],
      }
    }
  })
}
