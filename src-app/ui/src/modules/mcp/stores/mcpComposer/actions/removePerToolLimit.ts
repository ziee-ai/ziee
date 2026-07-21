import { resolveConfigKey } from '../state'
import type { McpComposerSet, McpComposerGet } from '../state'

/**
 * Remove a per-tool iteration limit.
 */
export default (set: McpComposerSet, _get: McpComposerGet) => (
  conversationId: string | null,
  serverId: string,
  toolName: string,
) => {
  set(state => {
    const configKey = resolveConfigKey(state, conversationId)
    const config = state.conversationConfigs.get(configKey)

    if (config && config.loopSettings?.per_tool_max_iteration) {
      config.loopSettings = {
        ...config.loopSettings,
        per_tool_max_iteration: config.loopSettings.per_tool_max_iteration.filter(
          t => !(t.server_id === serverId && t.tool_name === toolName)
        ),
      }
    }
  })
}
