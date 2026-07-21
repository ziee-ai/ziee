// Consumer → @ziee/notification-ui. The generic notification inbox store moved
// to the SDK (`createNotificationsStore`, reusable by any SDK-consuming app).
// This thin consumer binds ziee's concrete REST surface + read permission to
// the SDK factory, then re-exports `{ Notifications, useNotificationsStore }` at
// the SAME path so `module.tsx` (and the `types.ts` declaration merge) stay
// unchanged.
import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import {
  createNotificationsStore,
  notificationsSeam,
  type NotificationApiPort,
} from '@ziee/notification-ui'

export const Notifications = createNotificationsStore({
  // `ApiClient.Notification` is ziee's generated REST surface; it structurally
  // satisfies the SDK's `NotificationApiPort`. Cast at the seam boundary
  // (mirrors @ziee/shell's `Stores as unknown as {...}` typed-view casts) so the
  // SDK stays free of the app's generated types.
  api: ApiClient.Notification as unknown as NotificationApiPort,
  readPermission: Permissions.NotificationsRead,
  // Navigation seam — the SDK hardcodes zero routes; ziee supplies its own.
  // Kind-specific ids ride the `payload` jsonb column: a conversation-linked
  // notification opens the chat, everything else falls back to the inbox.
  onNavigate: (n, navigate) => {
    const conversationId = (n.payload as { conversation_id?: string } | null)
      ?.conversation_id
    navigate(conversationId ? `/chat/${conversationId}` : '/notifications')
  },
  inboxPath: '/notifications',
})

export const useNotificationsStore = Notifications.store

// SEAM: inject into the SDK notification widgets (replaces the old global Stores.Notifications).
notificationsSeam.set(Notifications as unknown as Parameters<typeof notificationsSeam.set>[0])
