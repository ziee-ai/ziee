import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import { scheduledTasksState, type ScheduledTasksState } from './state'
import type { Actions } from './actions.gen'

const ScheduledTasksDef = defineStore<ScheduledTasksState, Actions>('ScheduledTasks', {
  immer: true,
  state: scheduledTasksState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, get, actions }) => {
    const reload = () => {
      if (!hasPermissionNow(Permissions.SchedulerUse)) return
      void actions.loadTasks()
      // refresh the open task's runs, if any are loaded — refetch the SAME page.
      const meta = get().runsMetaByTask
      const loaded = Object.keys(get().runsByTask)
      for (const id of loaded)
        void actions.loadRuns(id, meta[id]?.page ?? 1, meta[id]?.perPage ?? 10)
    }
    on('sync:scheduled_task', reload)
    on('sync:reconnect', reload)
  },
})
export const ScheduledTasks = registerLazyStore(ScheduledTasksDef)
export const useScheduledTasksStore = ScheduledTasksDef.store
