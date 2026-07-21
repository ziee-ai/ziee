import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { litSearchAdminState, type LitSearchAdminState } from './state'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { Actions } from './actions.gen'

const LitSearchAdminDef = defineStore<LitSearchAdminState, Actions>('LitSearchAdmin', {
  immer: true,
  state: litSearchAdminState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, actions }) => {
    const reload = () => {
      if (!hasPermissionNow(Permissions.LitSearchAdminRead)) return
      void actions.load()
    }
    on('sync:lit_search_settings', reload)
    on('sync:reconnect', reload)
    reload()
  },
})
export const LitSearchAdmin = registerLazyStore(LitSearchAdminDef)
export const useLitSearchAdminStore = LitSearchAdminDef.store
