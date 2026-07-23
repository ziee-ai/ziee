// Consumer → @ziee/notification-ui. The generic notification inbox store moved
// to the SDK (`createNotificationsStore`, reusable by any SDK-consuming app).
// This thin consumer binds ziee's concrete REST surface + read permission to
// the SDK factory, then re-exports `{ Notifications, useNotificationsStore }` at
// the SAME path so `module.tsx` (and the `types.ts` declaration merge) stay
// unchanged.
import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { createStoreProxy } from '@ziee/framework/stores'
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

// Reactive proxy view (what the removed global `Stores.Notifications` resolved
// to) — for app pages that read fields / call actions reactively, e.g. the agent
// inbox page. The bare `Notifications` handle exposes no reactive fields.
export const NotificationsView = createStoreProxy(Notifications.store)

// SEAM: inject into the SDK notification widgets (replaces the old global
// Stores.Notifications). `createNotificationsStore` returns the defineStore
// HANDLE (no reactive fields), but the widgets read `.items`/`.unread`
// reactively — so inject a store PROXY (what `Stores.Notifications` used to
// resolve to), not the bare handle.
notificationsSeam.set(
  createStoreProxy(
    Notifications.store,
  ) as unknown as Parameters<typeof notificationsSeam.set>[0],
)
