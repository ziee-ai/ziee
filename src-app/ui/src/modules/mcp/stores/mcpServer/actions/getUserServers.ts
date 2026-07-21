import type { McpServer } from '@/api-client/types'
import type { McpServerGet } from '../state'

/** Return only non-system (user) servers. */
export default (_set: never, _get: McpServerGet) =>
  (servers: McpServer[]): McpServer[] =>
    servers.filter(server => !server.is_system)
