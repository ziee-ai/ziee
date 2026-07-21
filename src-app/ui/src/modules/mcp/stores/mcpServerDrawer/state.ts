import type { McpServer } from '@/api-client/types'
import type { StoreSet } from '@ziee/framework/store-kit'

export interface McpServerDrawerPrefill {
  fields: Partial<McpServer>
  hub_id?: string
}

export type McpServerDrawerMode = 'create' | 'edit' | 'clone' | 'create-system' | 'edit-system'

export const mcpServerDrawerState = {
  open: false,
  loading: false,
  editingServer: null as McpServer | null,
  prefillData: null as McpServerDrawerPrefill | null,
  isCloning: false,
  mode: 'create' as McpServerDrawerMode,
}

export type McpServerDrawerState = typeof mcpServerDrawerState
export type McpServerDrawerSet = StoreSet<McpServerDrawerState>
export type McpServerDrawerGet = () => McpServerDrawerState
