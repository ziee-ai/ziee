import type { SystemMcpServerGet, SystemMcpServerSet } from '../state'
import loadSystemServersFactory from './loadSystemServers'

export default (set: SystemMcpServerSet, get: SystemMcpServerGet) => {
  const loadSystemServers = loadSystemServersFactory(set, get)
  return async (): Promise<void> => {
    const { systemServersPage, systemServersPageSize } = get()
    await loadSystemServers(systemServersPage, systemServersPageSize)
  }
}
