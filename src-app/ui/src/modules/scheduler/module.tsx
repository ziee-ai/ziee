import { CalendarClock } from 'lucide-react'

import { Permissions } from '@/api-client/permissions'
import { createModule } from '@ziee/framework'
import { Stores } from '@ziee/framework/stores'
import { useDelayedFalse } from '@/hooks/useDelayedFalse'
import { AppLayoutDef } from '@/modules/layouts/app-layout'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import { useScheduledTasksStore } from './stores/scheduledTasks/index'
import { useSchedulerAdminStore } from './stores/schedulerAdmin'
import { useSchedulerDrawerStore } from './stores/SchedulerDrawer.store'
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
      // Top-level nav destination → render inside the app shell (left sidebar +
      // header bar), matching every other top-level page (chat/projects/
      // knowledge-base). Without this the page renders bare (no sidebar/header).
      layout: AppLayoutDef,
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
