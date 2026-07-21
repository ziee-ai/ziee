import type { McpComposerSet, McpComposerGet } from '../state'
import type { McpToolCall } from '../state'

/** Update an existing tool call. */
export default (set: McpComposerSet, _get: McpComposerGet) => (toolUseId: string, updates: Partial<McpToolCall>) => {
  set(state => {
    const existing = state.toolCalls.get(toolUseId)
    if (existing) {
      state.toolCalls.set(toolUseId, {
        ...existing,
        ...updates,
      })
    }
  })
}
