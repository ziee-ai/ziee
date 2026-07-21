import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { serverUpdateState, type ServerUpdateState } from './state'
import { hasPermissionNow } from '@/core/permissions'
import { Permissions } from '@/api-client/permissions'
import type { Actions } from './actions.gen'

const ServerUpdateDef = defineStore<ServerUpdateState, Actions>('ServerUpdate', {
  immer: true,
  state: serverUpdateState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ actions }) => {
    if (!hasPermissionNow(Permissions.ServerUpdateRead)) return
    void actions.loadStatus()
  },
})
export const ServerUpdate = registerLazyStore(ServerUpdateDef)
export const useServerUpdateStore = ServerUpdateDef.store
