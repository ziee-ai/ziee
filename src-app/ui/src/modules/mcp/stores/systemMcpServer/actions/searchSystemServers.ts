import { type McpServer } from '@/api-client/types'
import type { SystemMcpServerGet, SystemMcpServerSet } from '../state'

export default (_set: SystemMcpServerSet, get: SystemMcpServerGet) =>
  (query: string): McpServer[] => {
    const { systemServers } = get()
    if (!query.trim()) return systemServers
    const searchTerm = query.toLowerCase()
    return systemServers.filter(
      server =>
        server.name.toLowerCase().includes(searchTerm) ||
        server.display_name.toLowerCase().includes(searchTerm) ||
        server.description?.toLowerCase().includes(searchTerm) ||
        server.transport_type.toLowerCase().includes(searchTerm),
    )
  }
