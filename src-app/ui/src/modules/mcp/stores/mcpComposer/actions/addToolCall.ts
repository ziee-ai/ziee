import type { McpComposerSet, McpComposerGet } from '../state'
import type { McpToolCall } from '../state'

/** Add a new tool call. */
export default (set: McpComposerSet, _get: McpComposerGet) => (toolCall: McpToolCall) => {
  set(state => {
    state.toolCalls.set(toolCall.tool_use_id, toolCall)
  })
}
