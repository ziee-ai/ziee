import type { SystemMcpServerGet, SystemMcpServerSet } from '../state'
import loadSystemServersFactory from './loadSystemServers'

export default (set: SystemMcpServerSet, get: SystemMcpServerGet) => {
  const loadSystemServers = loadSystemServersFactory(set, get)
  return (status: string) => {
    set({ statusFilter: status, systemServersPage: 1 })
    void loadSystemServers(1)
  }
}
