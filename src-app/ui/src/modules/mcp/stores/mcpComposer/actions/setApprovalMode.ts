import { resolveConfigKey } from '../state'
import type { McpComposerSet, McpComposerGet } from '../state'

/**
 * Set approval mode for a conversation (or pending if conversationId is null).
 */
export default (set: McpComposerSet, _get: McpComposerGet) => (
  conversationId: string | null,
  mode: 'disabled' | 'auto_approve' | 'manual_approve',
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

    if (config) {
      config.approvalMode = mode
    }
  })
}
