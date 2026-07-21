import { type McpServer } from '@/api-client/types'
import type { SystemMcpServerGet, SystemMcpServerSet } from '../state'

export default (_set: SystemMcpServerSet, get: SystemMcpServerGet) =>
  (): McpServer[] =>
    get().systemServers.filter(server => server.enabled)
