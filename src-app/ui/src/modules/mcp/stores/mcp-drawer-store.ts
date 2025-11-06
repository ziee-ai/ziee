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

  // Actions
  openMcpServerDrawer: (
    server?: McpServer,
    mode?: 'create' | 'edit' | 'clone' | 'create-system' | 'edit-system',
  ) => void
  closeMcpServerDrawer: () => void
  setMcpServerDrawerLoading: (loading: boolean) => void
}

export const useMcpServerDrawerStore = create<McpServerDrawerState>()(
  subscribeWithSelector(
    (set): McpServerDrawerState => ({
      open: false,
      loading: false,
      editingServer: null,
      isCloning: false,
      mode: 'create',

      // Actions
      openMcpServerDrawer: (
        server?: McpServer,
        mode:
          | 'create'
          | 'edit'
          | 'clone'
          | 'create-system'
          | 'edit-system' = 'create',
      ) => {
        set({
          open: true,
          editingServer: server || null,
          isCloning: mode === 'clone',
          mode,
        })
      },

      closeMcpServerDrawer: () => {
        set({
          open: false,
          loading: false,
          editingServer: null,
          isCloning: false,
          mode: 'create',
        })
      },

      setMcpServerDrawerLoading: (loading: boolean) => {
        set({ loading })
      },
    }),
  ),
)
