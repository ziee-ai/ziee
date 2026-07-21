import type { McpComposerSet, McpComposerGet } from '../state'

/**
 * Resolve an elicitation request (accept/decline/cancel).
 */
export default (set: McpComposerSet, _get: McpComposerGet) => async (
  elicitation_id: string,
  action: 'accept' | 'decline' | 'cancel',
  content?: Record<string, unknown>,
) => {
  const finalStatus = action === 'accept' ? 'accepted' : action === 'decline' ? 'declined' : 'cancelled'

  // Optimistic update
  set(state => {
    const req = state.elicitationRequests.get(elicitation_id)
    if (req) {
      req.status = finalStatus
      if (action === 'accept' && content) {
        req.response_content = content
      }
    }
  })

  try {
    const { ApiClient } = await import('@/api-client')
    await ApiClient.Mcp.respondToElicitation({
      elicitation_id,
      action,
      ...(action === 'accept' && content ? { content } : {}),
    })
  } catch (e: unknown) {
    const status = e != null && typeof e === 'object' && 'status' in e
      ? (e as { status?: number }).status
      : undefined
    set(state => {
      const req = state.elicitationRequests.get(elicitation_id)
      if (!req) return
      // 404 = already gone → treat as cancelled. Otherwise roll back.
      req.status = status === 404 ? 'cancelled' : 'pending'
    })
    if (status !== 404) {
      console.error('[MCP Store] Failed to POST elicitation response:', e)
    }
  }
  // Note: we intentionally do NOT delete the entry — the component reads from McpComposer
  // as the live source of truth during streaming.
}
