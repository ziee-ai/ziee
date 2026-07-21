import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import { webSearchUserKeysState, type WebSearchUserKeysState } from './state'
import type { Actions } from './actions.gen'

const WebSearchUserKeysDef = defineStore<WebSearchUserKeysState, Actions>('WebSearchUserKeys', {
  immer: true,
  state: webSearchUserKeysState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, actions }) => {
    const reload = () => {
      if (!hasPermissionNow(Permissions.WebSearchUse)) return
      void actions.load()
    }
    on('sync:web_search_user_key', reload)
    on('sync:reconnect', reload)
    void actions.load()
  },
})
export const WebSearchUserKeys = registerLazyStore(WebSearchUserKeysDef)
export const useWebSearchUserKeysStore = WebSearchUserKeysDef.store
