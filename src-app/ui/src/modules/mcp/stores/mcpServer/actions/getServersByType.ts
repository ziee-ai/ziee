import type { McpServer } from '@/api-client/types'
import type { McpServerGet } from '../state'

/** Return servers matching a transport type (case-insensitive). */
export default (_set: never, _get: McpServerGet) =>
  (servers: McpServer[], transportType: string): McpServer[] =>
    servers.filter(
      server => server.transport_type.toLowerCase() === transportType.toLowerCase(),
    )
