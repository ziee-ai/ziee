import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import type { McpServer } from '@/api-client/types'
import { Stores } from '@/core/stores'

/**
 * Pre-fill payload for the "Install from Hub" flow. The Hub MCP
 * card's "Install" / "Install for the system" buttons call
 * `openMcpServerDrawer(undefined, mode, prefillData)` with the
 * manifest's field defaults so the drawer opens fully populated.
 * The user reviews / fills in secrets, then submits via the normal
 * create endpoint with `hub_id` in the request body so the backend
 * still records the install in `hub_entities`.
 */
export interface McpServerDrawerPrefill {
  fields: Partial<McpServer>
  hub_id?: string
}

// MCP Server Drawer State
interface McpServerDrawerState {
  open: boolean
  loading: boolean
  editingServer: McpServer | null
  prefillData: McpServerDrawerPrefill | null
  isCloning: boolean
  mode: 'create' | 'edit' | 'clone' | 'create-system' | 'edit-system'

  // Actions
  openMcpServerDrawer: (
    server?: McpServer,
    mode?: 'create' | 'edit' | 'clone' | 'create-system' | 'edit-system',
    prefillData?: McpServerDrawerPrefill,
  ) => void
  closeMcpServerDrawer: () => void
  setMcpServerDrawerLoading: (loading: boolean) => void

  // Initialization
  __init__: {
    __store__: () => void
  }
  __destroy__?: () => void
}

export const useMcpServerDrawerStore = create<McpServerDrawerState>()(
  subscribeWithSelector(
    (set, get): McpServerDrawerState => ({
      open: false,
      loading: false,
      editingServer: null,
      prefillData: null,
      isCloning: false,
      mode: 'create',

      __init__: {
        __store__: () => {
          const GROUP = 'McpServerDrawerStore'
          const eventBus = Stores.EventBus

          // Subscribe to mcp_server.updated
          eventBus.on(
            'mcp_server.updated',
            async event => {
              const { server } = event.data
              const state = get()

              if (
                (state.mode === 'edit' || state.mode === 'edit-system') &&
                state.editingServer?.id === server.id
              ) {
                set({ editingServer: server })
              }
            },
            GROUP,
          )

          // Subscribe to mcp_server.deleted
          eventBus.on(
            'mcp_server.deleted',
            async event => {
              const { serverId } = event.data
              const state = get()

              if (state.editingServer?.id === serverId) {
                get().closeMcpServerDrawer()
              }
            },
            GROUP,
          )
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
        prefillData?: McpServerDrawerPrefill,
      ) => {
        set({
          open: true,
          editingServer: server || null,
          prefillData: prefillData ?? null,
          isCloning: mode === 'clone',
          mode,
        })
      },

      closeMcpServerDrawer: () => {
        set({
          open: false,
          loading: false,
          editingServer: null,
          prefillData: null,
          isCloning: false,
          mode: 'create',
        })
      },

      setMcpServerDrawerLoading: (loading: boolean) => {
        set({ loading })
      },

      __destroy__: () => {
        Stores.EventBus.removeGroupListeners('McpServerDrawerStore')
      },
    }),
  ),
)
