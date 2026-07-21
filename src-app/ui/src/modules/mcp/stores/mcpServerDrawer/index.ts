import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { mcpServerDrawerState, type McpServerDrawerState } from './state'
import type { Actions } from './actions.gen'

const McpServerDrawerDef = defineStore<McpServerDrawerState, Actions>('McpServerDrawer', {
  immer: true,
  state: mcpServerDrawerState,
  actions: import.meta.glob('./actions/*.ts'),
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

export const McpServerDrawer = registerLazyStore(McpServerDrawerDef)
export const useMcpServerDrawerStore = McpServerDrawerDef.store

// Raw store definition export for compatibility.
export { McpServerDrawerDef }
