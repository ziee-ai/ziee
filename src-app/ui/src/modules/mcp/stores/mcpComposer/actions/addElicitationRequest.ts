import type { McpComposerSet, McpComposerGet } from '../state'
import type { SSEChatStreamMcpElicitationRequiredData } from '@/api-client/types'

/**
 * Add an elicitation request.
 */
export default (set: McpComposerSet, _get: McpComposerGet) => (request: SSEChatStreamMcpElicitationRequiredData) => {
  set(state => {
    state.elicitationRequests.set(request.elicitation_id, {
      ...request,
      status: 'pending',
    })
  })
}
