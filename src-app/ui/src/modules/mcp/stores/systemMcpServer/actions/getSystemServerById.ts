import { type McpServer } from '@/api-client/types'
import type { SystemMcpServerGet, SystemMcpServerSet } from '../state'

export default (_set: SystemMcpServerSet, get: SystemMcpServerGet) =>
  (serverId: string): McpServer | null =>
    get().systemServers.find(server => server.id === serverId) || null
