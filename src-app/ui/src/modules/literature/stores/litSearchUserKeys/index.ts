import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { litSearchUserKeysState, type LitSearchUserKeysState } from './state'
import type { Actions } from './actions.gen'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'

const LitSearchUserKeysDef = defineStore<LitSearchUserKeysState, Actions>('LitSearchUserKeys', {
  immer: true,
  state: litSearchUserKeysState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, actions }) => {
    const reload = () => {
      if (!hasPermissionNow(Permissions.LitSearchUse)) return
      void actions.load()
    }
    on('sync:lit_search_user_key', reload)
    on('sync:reconnect', reload)
    void actions.load()
  },
})
export const LitSearchUserKeys = registerLazyStore(LitSearchUserKeysDef)
export const useLitSearchUserKeysStore = LitSearchUserKeysDef.store
