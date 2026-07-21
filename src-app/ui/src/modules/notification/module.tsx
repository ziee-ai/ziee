import { Permissions } from '@/api-client/permissions'
import { createModule } from '@ziee/framework'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import { usePermission } from '@/core/permissions'

import { NotificationBellWidget } from './components/NotificationBellWidget'
import { useNotificationsStore } from './stores/Notifications.store'
import '@/modules/notification/types' // register Notifications (declaration merge)
import '@/modules/notification/kinds' // register ziee's notification kinds/renderers (SDK seam)

const NotificationsPage = lazyWithPreload(() =>
  import('./pages/NotificationsPage').then(m => ({
    default: m.NotificationsPage,
  })),
)
const NotificationToastListener = lazyWithPreload(() =>
  import('./components/NotificationToastListener').then(m => ({
    default: m.NotificationToastListener,
  })),
)

export default createModule({
  metadata: {
    name: 'notification',
    version: '1.0.0',
    description: 'Notification inbox',
  },
  // smart-loading gate (build-lifted into the manifest)
  shouldLoad: (ctx) => ctx.isAuthenticated,
  dependencies: ['router'],
  routes: [
    {
      path: '/notifications',
      element: NotificationsPage,
      requiresAuth: true,
      permission: Permissions.NotificationsRead,
    },
  ],
  stores: [{ name: 'Notifications', store: useNotificationsStore }],
  components: [
    {
      id: 'notification-toast-listener',
      component: NotificationToastListener,
      // Gate: notifications are per-user (`notifications::read`, held by every
      // authenticated user). A logged-out visitor has no notifications, so don't
      // load the toast-listener chunk on the login page.
      shouldMount: () => usePermission(Permissions.NotificationsRead),
      order: 90,
    },
  ],
  slots: {
    sidebarBottom: [
      {
        id: 'notification-bell',
        component: NotificationBellWidget,
        order: 5,
        // Gate: the bell + list render the user's notifications (backed by
        // `notifications::read`). Match the data's read perm so a role
        // without the grant sees neither the bell nor a 403 fetch.
        permission: Permissions.NotificationsRead,
      },
    ],
  },
})
