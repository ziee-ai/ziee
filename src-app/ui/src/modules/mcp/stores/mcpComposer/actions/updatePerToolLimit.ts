import { resolveConfigKey } from '../state'
import type { McpComposerSet, McpComposerGet } from '../state'

/**
 * Update a per-tool iteration limit.
 */
export default (set: McpComposerSet, _get: McpComposerGet) => (
  conversationId: string | null,
  serverId: string,
  toolName: string,
  maxIteration: number,
) => {
  set(state => {
    const configKey = resolveConfigKey(state, conversationId)
    const config = state.conversationConfigs.get(configKey)

    if (config && config.loopSettings?.per_tool_max_iteration) {
      config.loopSettings = {
        ...config.loopSettings,
        per_tool_max_iteration: config.loopSettings.per_tool_max_iteration.map(t =>
          t.server_id === serverId && t.tool_name === toolName
            ? { ...t, max_iteration: maxIteration }
            : t
        ),
      }
    }
  })
}
