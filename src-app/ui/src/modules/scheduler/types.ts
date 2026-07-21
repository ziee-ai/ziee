import type { StoreProxy } from '@ziee/framework/stores'

import type { useSchedulerAdminStore } from './stores/schedulerAdmin'
import type { useSchedulerDrawerStore } from './stores/schedulerDrawer'
import type { useScheduledTasksStore } from './stores/scheduledTasks/index'

declare module '@ziee/framework/stores' {
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
