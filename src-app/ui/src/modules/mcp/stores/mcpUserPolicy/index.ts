import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { mcpUserPolicyState, type McpUserPolicyState } from './state'
import type { Actions } from './actions.gen'

const McpUserPolicyDef = defineStore<McpUserPolicyState, Actions>('McpUserPolicy', {
  immer: true,
  state: mcpUserPolicyState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, actions }) => {
    // Cross-device sync. `load` self-gates on `mcp_servers::read` internally
    // (returns early when the user lacks it), satisfying the no-403 reconnect
    // rule — `sync:reconnect` fires for every store regardless of audience.
    const reload = () => void actions.load()
    on('sync:mcp_user_policy', reload)
    on('sync:reconnect', reload)
    void actions.load()
  },
})

export const McpUserPolicy = registerLazyStore(McpUserPolicyDef)
export const useMcpUserPolicyStore = McpUserPolicyDef.store
export { McpUserPolicyDef }

declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    McpUserPolicy: import('@ziee/framework/stores').StoreProxy<ReturnType<typeof McpUserPolicyDef.store.getState>>
  }
}
