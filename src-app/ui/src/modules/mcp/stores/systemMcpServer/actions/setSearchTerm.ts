import type { SystemMcpServerGet, SystemMcpServerSet } from '../state'
import loadSystemServersFactory from './loadSystemServers'

/** Debounce timer for system MCP search-term reloads (250ms). */
let sysMcpSearchDebounce: ReturnType<typeof setTimeout> | null = null

export default (set: SystemMcpServerSet, get: SystemMcpServerGet) => {
  const loadSystemServers = loadSystemServersFactory(set, get)
  return (q: string) => {
    set({ searchTerm: q, systemServersPage: 1 })
    if (sysMcpSearchDebounce) clearTimeout(sysMcpSearchDebounce)
    sysMcpSearchDebounce = setTimeout(() => {
      void loadSystemServers(1)
    }, 250)
  }
}
