import type { McpComposerGet } from '../state'
import type { McpToolCall } from '../state'

/** Get all active tool calls (started or pending approval) — synchronous. */
export default (_set: unknown, get: McpComposerGet): () => McpToolCall[] => {
  return () => {
    const state = get()
    const allCalls = Array.from(state.toolCalls.values())
    return allCalls.filter(
      call => call.status === 'started' || call.status === 'pending_approval',
    )
  }
}
