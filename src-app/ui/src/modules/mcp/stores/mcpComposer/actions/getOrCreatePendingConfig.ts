import { PENDING_CONVERSATION_KEY } from '../../approvalRouting'
import type { McpComposerSet, McpComposerGet } from '../state'

interface ConversationMcpConfig {
  selectedServers: Map<string, { server_id: string; tools: string[] }>
  disabledServers?: import('@/api-client/types').DisabledServer[]
  approvalMode?: 'disabled' | 'auto_approve' | 'manual_approve'
  autoApprovedTools?: import('@/api-client/types').AutoApprovedServer[]
  loopSettings?: import('@/api-client/types').LoopSettings
}

/**
 * Get or create pending config for new conversations.
 */
export default (set: McpComposerSet, get: McpComposerGet): () => ConversationMcpConfig => {
  return (): ConversationMcpConfig => {
    const state = get()
    let config = state.conversationConfigs.get(PENDING_CONVERSATION_KEY)
    if (!config) {
      config = {
        selectedServers: new Map(),
        disabledServers: [],
        approvalMode: 'manual_approve',
        autoApprovedTools: [],
      }
      set(s => {
        s.conversationConfigs.set(PENDING_CONVERSATION_KEY, config!)
      })
    }
    return config
  }
}
