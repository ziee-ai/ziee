import type { McpToolCallsGet, McpToolCallsSet } from '../state'
import loadCallsFactory from './loadCalls'

export default (set: McpToolCallsSet, get: McpToolCallsGet) => {
  const loadCalls = loadCallsFactory(set, get)
  return async (hide: boolean) => {
    set(draft => {
      draft.hideBuiltIn = hide
      draft.currentPage = 1
    })
    void loadCalls(undefined, 1)
  }
}
