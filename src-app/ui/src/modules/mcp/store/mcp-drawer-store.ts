import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import type { McpServer } from '@/api-client/types'

// MCP Server Drawer State
interface McpServerDrawerState {
  open: boolean
  loading: boolean
  editingServer: McpServer | null
  isCloning: boolean
  mode: 'create' | 'edit' | 'clone' | 'create-system' | 'edit-system'
}

export const useMcpServerDrawerStore = create<McpServerDrawerState>()(
  subscribeWithSelector(
    (): McpServerDrawerState => ({
      open: false,
      loading: false,
      editingServer: null,
      isCloning: false,
      mode: 'create',
    }),
  ),
)

// MCP Server Drawer Actions
export const openMcpServerDrawer = (
  server?: McpServer,
  mode:
    | 'create'
    | 'edit'
    | 'clone'
    | 'create-system'
    | 'edit-system' = 'create',
) => {
  useMcpServerDrawerStore.setState({
    open: true,
    editingServer: server || null,
    isCloning: mode === 'clone',
    mode,
  })
}

export const closeMcpServerDrawer = () => {
  useMcpServerDrawerStore.setState({
    open: false,
    loading: false,
    editingServer: null,
    isCloning: false,
    mode: 'create',
  })
}

export const setMcpServerDrawerLoading = (loading: boolean) => {
  useMcpServerDrawerStore.setState({ loading })
}
