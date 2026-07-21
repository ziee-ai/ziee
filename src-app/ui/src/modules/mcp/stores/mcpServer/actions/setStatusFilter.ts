import type { McpServerGet, McpServerSet } from '../state'
import loadMcpServersFactory from './loadMcpServers'

export default (set: McpServerSet, get: McpServerGet) => {
  const loadMcpServers = loadMcpServersFactory(set, get)

  return async (status: string) => {
    set(draft => {
      draft.statusFilter = status
      draft.currentPage = 1
    })
    void loadMcpServers(1)
  }
}
