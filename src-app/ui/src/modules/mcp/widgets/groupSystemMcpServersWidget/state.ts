import type { McpServer } from '@/api-client/types'
import type { StoreSet } from '@ziee/framework/store-kit'

interface GroupServers {
  groupId: string
  servers: McpServer[]
  loading: boolean
  error: string | null
  lastFetched: number | null
}

export const groupSystemMcpServersWidgetState = {
  // Map of groupId -> server data
  groupServers: new Map<string, GroupServers>(),
  // Cached servers
  allServers: [] as McpServer[],
  serversLoading: false,
  serversError: null as string | null,
  serversInitialized: false,
}

export type GroupSystemMcpServersWidgetState = typeof groupSystemMcpServersWidgetState
export type GroupSystemMcpServersWidgetSet = StoreSet<GroupSystemMcpServersWidgetState>
export type GroupSystemMcpServersWidgetGet = () => GroupSystemMcpServersWidgetState
