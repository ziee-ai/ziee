import { Inbox } from 'lucide-react'

import { Permissions } from '@/api-client/types'
import { createModule } from '@ziee/framework'
import { lazyWithPreload } from '@/utils/lazyWithPreload'

import { NotificationBellWidget } from './components/NotificationBellWidget'
import { useNotificationsStore } from './stores/Notifications.store'
import '@/modules/notification/types' // register Stores.Notifications (declaration merge)
import '@/modules/notification/kinds' // register ziee's notification kinds/renderers (SDK seam)

const NotificationsPage = lazyWithPreload(() =>
  import('./pages/NotificationsPage').then(m => ({
    default: m.NotificationsPage,
  })),
)
const AgentInboxPage = lazyWithPreload(() =>
  import('./pages/AgentInboxPage').then(m => ({
    default: m.AgentInboxPage,
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
  dependencies: ['router'],
  routes: [
    {
      path: '/notifications',
      element: NotificationsPage,
      requiresAuth: true,
      permission: Permissions.NotificationsRead,
    },
    {
      // Agent/background inbox (ITEM-26) — a focused view over the same
      // notifications, narrowed to background sub-agent / scheduled-loop results.
      // Same read perm as the inbox (self-gated store; no 403 for a role without
      // the grant).
      path: '/notifications/background',
      element: AgentInboxPage,
      requiresAuth: true,
      permission: Permissions.NotificationsRead,
    },
  ],
  stores: [{ name: 'Notifications', store: useNotificationsStore }],
  components: [
    {
      id: 'notification-toast-listener',
      component: NotificationToastListener,
      order: 90,
    },
  ],
  slots: {
    sidebarNavigation: [
      {
        // Discoverable entry point to the agent/background inbox (ITEM-26).
        // Mirrors the scheduler's `sidebarNavigation` entry exactly (id/icon/
        // label/path/order/permission) — a top-level nav destination for the
        // "Background results" page at `/notifications/background`.
        id: 'agent-inbox',
        icon: <Inbox />,
        label: 'Background results',
        path: '/notifications/background',
        // Sits just after "Scheduled Tasks" (order 22) — background/agent work
        // grouped together in the nav.
        order: 24,
        // Gate: SAME read perm as the `/notifications/background` route + the
        // bell/inbox data (`notifications::read`). A user without the grant
        // never sees the entry (and the page self-gates its fetch → no 403).
        permission: Permissions.NotificationsRead,
      },
    ],
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
