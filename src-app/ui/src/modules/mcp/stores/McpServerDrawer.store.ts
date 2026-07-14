import type { McpServer } from '@/api-client/types'
import { defineStore } from '@ziee/framework/store-kit'

/**
 * Pre-fill payload for the "Install from Hub" flow. The Hub MCP card's Install
 * buttons call `openMcpServerDrawer(undefined, mode, prefillData)` with the
 * manifest's field defaults so the drawer opens fully populated. The user
 * reviews / fills in secrets, then submits via the normal create endpoint with
 * `hub_id` in the body so the backend still records the install in `hub_entities`.
 */
export interface McpServerDrawerPrefill {
  fields: Partial<McpServer>
  hub_id?: string
}

type McpServerDrawerMode =
  | 'create'
  | 'edit'
  | 'clone'
  | 'create-system'
  | 'edit-system'

export const McpServerDrawer = defineStore('McpServerDrawer', {
  state: {
    open: false,
    loading: false,
    editingServer: null as McpServer | null,
    prefillData: null as McpServerDrawerPrefill | null,
    isCloning: false,
    mode: 'create' as McpServerDrawerMode,
  },
  actions: set => ({
    openMcpServerDrawer: (
      server?: McpServer,
      mode: McpServerDrawerMode = 'create',
      prefillData?: McpServerDrawerPrefill,
    ) =>
      set({
        open: true,
        editingServer: server || null,
        prefillData: prefillData ?? null,
        isCloning: mode === 'clone',
        mode,
      }),
    closeMcpServerDrawer: () =>
      set({
        open: false,
        loading: false,
        editingServer: null,
        prefillData: null,
        isCloning: false,
        mode: 'create',
      }),
    setMcpServerDrawerLoading: (loading: boolean) => set({ loading }),
  }),
  init: ({ on, get, set, actions }) => {
    on('mcp_server.updated', event => {
      const state = get()
      if (
        (state.mode === 'edit' || state.mode === 'edit-system') &&
        state.editingServer?.id === event.data.server.id
      ) {
        set({ editingServer: event.data.server })
      }
    })
    on('mcp_server.deleted', event => {
      if (get().editingServer?.id === event.data.serverId) actions.closeMcpServerDrawer()
    })
  },
})

export const useMcpServerDrawerStore = McpServerDrawer.store
