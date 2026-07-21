import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'

import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { schedulerAdminState, type SchedulerAdminState } from './state'
import type { Actions } from './actions.gen'

const SchedulerAdminDef = defineStore<SchedulerAdminState, Actions>('SchedulerAdmin', {
  immer: true,
  state: schedulerAdminState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, actions }) => {
    const reload = () => {
      if (!hasPermissionNow(Permissions.SchedulerAdminRead)) return
      void actions.loadSettings()
    }
    on('sync:scheduler_admin_settings', reload)
    on('sync:reconnect', reload)
    reload()
  },
})
export const SchedulerAdmin = registerLazyStore(SchedulerAdminDef)
export const useSchedulerAdminStore = SchedulerAdminDef.store
