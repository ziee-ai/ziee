import type { McpToolCallsGet, McpToolCallsSet } from '../state'
import loadCallsFactory from './loadCalls'

export default (set: McpToolCallsSet, get: McpToolCallsGet) => {
  const loadCalls = loadCallsFactory(set, get)
  return async (page: number, pageSize?: number) => {
    void loadCalls(undefined, page, pageSize)
  }
}
