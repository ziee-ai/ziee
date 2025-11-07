import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import type { McpServer } from '@/api-client/types'
import { Stores } from '@/core/stores'

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

  // Initialization
  __init__: {
    __store__: () => void
  }
}

export const useMcpServerDrawerStore = create<McpServerDrawerState>()(
  subscribeWithSelector(
    (set, get): McpServerDrawerState => ({
      open: false,
      loading: false,
      editingServer: null,
      isCloning: false,
      mode: 'create',

      __init__: {
        __store__: () => {
          const eventBus = Stores.EventBus

          // Subscribe to mcp_server.updated
          eventBus.on('mcp_server.updated', async event => {
            const { server } = event.data
            const state = get()

            if (
              (state.mode === 'edit' || state.mode === 'edit-system') &&
              state.editingServer?.id === server.id
            ) {
              set({ editingServer: server })
            }
          })

          // Subscribe to mcp_server.deleted
          eventBus.on('mcp_server.deleted', async event => {
            const { serverId } = event.data
            const state = get()

            if (state.editingServer?.id === serverId) {
              get().closeMcpServerDrawer()
            }
          })
        },
      },

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
