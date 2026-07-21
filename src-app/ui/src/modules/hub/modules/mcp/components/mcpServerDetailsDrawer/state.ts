import type { StoreSet } from '@ziee/framework/store-kit'
import type { HubMCPServer } from '@/api-client/types'

export const mcpServerDetailsDrawerState = {
  isOpen: false,
  selectedServer: null as HubMCPServer | null,
  /** True while the fresh manifest is being fetched on open. */
  loading: false,
}

export type McpServerDetailsDrawerState = typeof mcpServerDetailsDrawerState
export type McpServerDetailsDrawerSet = StoreSet<McpServerDetailsDrawerState>
export type McpServerDetailsDrawerGet = () => McpServerDetailsDrawerState
