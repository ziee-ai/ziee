import { CalendarClock } from 'lucide-react'

import { Permissions } from '@/api-client/types'
import { createModule } from '@/core'
import { Stores } from '@/core/stores'
import { useDelayedFalse } from '@/hooks/useDelayedFalse'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'

import { useSchedulerAdminStore } from './stores/SchedulerAdmin.store'
import { useSchedulerDrawerStore } from './stores/SchedulerDrawer.store'
import { useScheduledTasksStore } from './stores/ScheduledTasks.store'
import '@/modules/scheduler/types' // register Stores.* (declaration merge)
import '@/modules/settings/types/SettingsSlots'

const ScheduledTasksPage = lazyWithPreload(() =>
  import('./pages/ScheduledTasksPage').then(m => ({
    default: m.ScheduledTasksPage,
  })),
)
const SchedulerAdminPage = lazyWithPreload(() =>
  import('./pages/SchedulerAdminPage').then(m => ({
    default: m.SchedulerAdminPage,
  })),
)
const ScheduledTaskFormDrawer = lazyWithPreload(() =>
  import('./components/ScheduledTaskFormDrawer').then(m => ({
    default: m.ScheduledTaskFormDrawer,
  })),
)

export default createModule({
  metadata: {
    name: 'scheduler',
    version: '1.0.0',
    description: 'Scheduled / recurring tasks',
  },
  dependencies: ['router'],
  routes: [
    {
      path: '/scheduled-tasks',
      element: ScheduledTasksPage,
      requiresAuth: true,
      permission: Permissions.SchedulerUse,
    },
    {
      path: '/settings/scheduler',
      element: SchedulerAdminPage,
      requiresAuth: true,
      permission: Permissions.SchedulerAdminRead,
      layout: SettingsLayoutDef,
    },
  ],
  stores: [
    { name: 'ScheduledTasks', store: useScheduledTasksStore },
    { name: 'SchedulerAdmin', store: useSchedulerAdminStore },
    { name: 'SchedulerDrawer', store: useSchedulerDrawerStore },
  ],
  components: [
    {
      id: 'scheduled-task-form-drawer',
      component: ScheduledTaskFormDrawer,
      shouldMount: () => useDelayedFalse(() => Stores.SchedulerDrawer.open),
      order: 100,
    },
  ],
  slots: {
    sidebarNavigation: [
      {
        id: 'scheduled-tasks',
        icon: <CalendarClock />,
        label: 'Scheduled Tasks',
        path: '/scheduled-tasks',
        order: 22,
        permission: Permissions.SchedulerUse,
      },
    ],
    settingsAdminPages: [
      {
        id: 'scheduler',
        icon: <CalendarClock />,
        label: 'Scheduler',
        path: 'scheduler',
        order: 30,
        permission: Permissions.SchedulerAdminRead,
      },
    ],
  },
})
