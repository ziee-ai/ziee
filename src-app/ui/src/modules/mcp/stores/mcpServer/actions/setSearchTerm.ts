import type { McpServerGet, McpServerSet } from '../state'
import loadMcpServersFactory from './loadMcpServers'

/** Debounce timer for search-term reloads (250ms). */
let mcpSearchDebounce: ReturnType<typeof setTimeout> | null = null

export default (set: McpServerSet, get: McpServerGet) => {
  const loadMcpServers = loadMcpServersFactory(set, get)

  return async (q: string) => {
    set(draft => {
      draft.searchTerm = q
      draft.currentPage = 1
    })
    if (mcpSearchDebounce) clearTimeout(mcpSearchDebounce)
    mcpSearchDebounce = setTimeout(() => {
      void loadMcpServers(1)
    }, 250)
  }
}
