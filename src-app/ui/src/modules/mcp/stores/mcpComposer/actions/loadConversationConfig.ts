import type { McpComposerSet, McpComposerGet } from '../state'

interface ConversationMcpConfig {
  selectedServers: Map<string, { server_id: string; tools: string[] }>
  disabledServers?: import('@/api-client/types').DisabledServer[]
  approvalMode?: 'disabled' | 'auto_approve' | 'manual_approve'
  autoApprovedTools?: import('@/api-client/types').AutoApprovedServer[]
  loopSettings?: import('@/api-client/types').LoopSettings
}

/**
 * Load conversation config (from backend or create default).
 */
export default (set: McpComposerSet, _get: McpComposerGet) => (
  conversationId: string,
  config?: ConversationMcpConfig,
) => {
  set(state => {
    if (config) {
      state.conversationConfigs.set(conversationId, config)
    } else {
      // Create default config
      state.conversationConfigs.set(conversationId, {
        selectedServers: new Map(),
        approvalMode: 'manual_approve',
        autoApprovedTools: [],
      })
    }

    // If this is current conversation, update selectedServers
    if (state.currentConversationId === conversationId) {
      const loadedConfig = state.conversationConfigs.get(conversationId)!
      state.selectedServers = new Map(loadedConfig.selectedServers)
    }
  })
  console.log('[MCP Store] Loaded conversation config:', conversationId)
}
