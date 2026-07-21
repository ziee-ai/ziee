import { resolveConfigKey } from '../state'
import type { McpComposerSet, McpComposerGet } from '../state'

/**
 * Set loop settings (partial update).
 */
export default (set: McpComposerSet, _get: McpComposerGet) => (
  conversationId: string | null,
  settings: Partial<import('@/api-client/types').LoopSettings>,
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

    config.loopSettings = { ...config.loopSettings, ...settings }
  })
}
