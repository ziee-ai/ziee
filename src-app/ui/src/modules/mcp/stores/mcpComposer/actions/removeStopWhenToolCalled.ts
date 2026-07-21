import { resolveConfigKey } from '../state'
import type { McpComposerSet, McpComposerGet } from '../state'

/**
 * Remove a tool from stop_when_tools_called.
 */
export default (set: McpComposerSet, _get: McpComposerGet) => (
  conversationId: string | null,
  serverId: string,
  toolName: string,
) => {
  set(state => {
    const configKey = resolveConfigKey(state, conversationId)
    const config = state.conversationConfigs.get(configKey)

    if (config && config.loopSettings?.stop_when_tools_called) {
      config.loopSettings = {
        ...config.loopSettings,
        stop_when_tools_called: config.loopSettings.stop_when_tools_called.filter(
          t => !(t.server_id === serverId && t.tool_name === toolName)
        ),
      }
    }
  })
}
