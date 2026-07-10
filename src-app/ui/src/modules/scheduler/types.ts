import type { StoreProxy } from '@/core/stores'

import type { useSchedulerAdminStore } from './stores/SchedulerAdmin.store'
import type { useSchedulerDrawerStore } from './stores/SchedulerDrawer.store'
import type { useScheduledTasksStore } from './stores/ScheduledTasks.store'

declare module '@/core/stores' {
  interface RegisteredStores {
    ScheduledTasks: StoreProxy<
      ReturnType<typeof useScheduledTasksStore.getState>
    >
    SchedulerAdmin: StoreProxy<
      ReturnType<typeof useSchedulerAdminStore.getState>
    >
    SchedulerDrawer: StoreProxy<
      ReturnType<typeof useSchedulerDrawerStore.getState>
    >
  }
}

export {}
