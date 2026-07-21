import type { StoreSet } from '@ziee/framework/store-kit'
import type { McpToolCall } from '@/api-client/types'

export const mcpToolCallsState = {
  calls: [] as McpToolCall[],
  total: 0,
  currentPage: 1,
  pageSize: 20,
  serverIdFilter: null as string | null,
  hideBuiltIn: false,
  loading: false,
  error: null as string | null,
}

export type McpToolCallsState = typeof mcpToolCallsState
export type McpToolCallsSet = StoreSet<McpToolCallsState>
export type McpToolCallsGet = () => McpToolCallsState
