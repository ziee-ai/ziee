import type { McpComposerGet } from '../state'

/**
 * Get conversation MCP settings from backend.
 */
export default (_set: unknown, _get: McpComposerGet) => async (conversationId: string) => {
  const { ApiClient } = await import('@/api-client')
  return await ApiClient.Conversation.getMcpSettings({
    id: conversationId,
  })
}
