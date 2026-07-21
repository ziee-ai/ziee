import type { McpServer } from '@/api-client/types'
import type { McpServerGet } from '../state'

/** Case-insensitive search across name, display_name, description, transport_type. */
export default (_set: never, _get: McpServerGet) =>
  (servers: McpServer[], query: string): McpServer[] => {
    if (!query.trim()) return servers
    const searchTerm = query.toLowerCase()
    return servers.filter(
      server =>
        server.name.toLowerCase().includes(searchTerm) ||
        server.display_name.toLowerCase().includes(searchTerm) ||
        server.description?.toLowerCase().includes(searchTerm) ||
        server.transport_type.toLowerCase().includes(searchTerm),
    )
  }
