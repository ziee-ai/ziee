import type { McpToolCallsGet, McpToolCallsSet } from '../state'

export default (set: McpToolCallsSet, _get: McpToolCallsGet) =>
  async () => {
    set(draft => {
      draft.error = null
    })
  }
