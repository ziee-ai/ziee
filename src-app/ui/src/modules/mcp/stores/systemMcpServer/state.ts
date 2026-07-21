import type { StoreSet } from '@ziee/framework/store-kit'
import type { McpServer } from '@/api-client/types'

/** Base state, reused by `init`'s reset-on-cleanup. */
export const INITIAL = {
  systemServers: [] as McpServer[],
  systemServersTotal: 0,
  systemServersPage: 1,
  systemServersPageSize: 20,
  systemServersInitialized: false,
  // Filter state (server-side). Search is debounced; status fires immediately.
  searchTerm: '',
  statusFilter: 'all',
  systemServersLoading: false,
  creating: false,
  updating: false,
  deleting: false,
  operationsLoading: new Map<string, boolean>(),
  systemServersError: null as string | null,
}

export const systemMcpServerState = {
  ...INITIAL,
  // Wait 10s before destroying (users might come back). Read by the proxy.
  __destroyDelay__: 10000,
}

export type SystemMcpServerState = typeof systemMcpServerState
export type SystemMcpServerSet = StoreSet<SystemMcpServerState>
export type SystemMcpServerGet = () => SystemMcpServerState
