import type { StoreSet } from '@ziee/framework/store-kit'
import type { McpServer } from '@/api-client/types'

export const mcpServerState = {
  // Accessible servers (personal + system from groups)
  servers: [] as McpServer[],
  isInitialized: false,
  // Pagination (defaults match the settings page's pageSizeOptions).
  currentPage: 1,
  pageSize: 10,
  total: 0,
  // Filter state (server-side). Search debounced; status immediate.
  searchTerm: '',
  statusFilter: 'all',
  loading: false,
  creating: false,
  updating: false,
  deleting: false,
  operationsLoading: new Map<string, boolean>(),
  error: null as string | null,
}

export type McpServerState = typeof mcpServerState
export type McpServerSet = StoreSet<McpServerState>
export type McpServerGet = () => McpServerState
