import type { McpComposerGet } from '../state'
import type { McpToolCall } from '../state'

/** Get a tool call by ID (synchronous selector). */
export default (_set: unknown, get: McpComposerGet): (toolUseId: string) => McpToolCall | undefined => {
  return (toolUseId: string) => {
    const state = get()
    return state.toolCalls.get(toolUseId)
  }
}
