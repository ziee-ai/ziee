import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { mcpToolCallsState, type McpToolCallsState } from './state'
import type { Actions } from './actions.gen'

const McpToolCallsDef = defineStore<McpToolCallsState, Actions>('McpToolCalls', {
  immer: true,
  state: mcpToolCallsState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, get, actions }) => {
    const reload = () => {
      const s = get()
      void actions.loadCalls(s.serverIdFilter, s.currentPage)
    }
    on('sync:mcp_tool_call', reload)
    on('sync:reconnect', reload)
  },
})
export const McpToolCalls = registerLazyStore(McpToolCallsDef)
export const useMcpToolCallsStore = McpToolCallsDef.store
