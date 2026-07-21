import type { McpComposerSet, McpComposerGet } from '../state'

/** Clear all tool calls for current conversation. */
export default (set: McpComposerSet, _get: McpComposerGet) => () => {
  set(state => {
    state.toolCalls.clear()
  })
}
