import { useEffect } from 'react'

import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/types'
import { message } from '@/components/ui'
import { hasPermissionNow } from '@/core/permissions'
import { Stores } from '@/core/stores'

/**
 * Globally-mounted (route-independent) listener that raises a live toast when a
 * new notification arrives — but only when its `interrupt` flag is set (a
 * `silent`/`notify_mode=silent` task's result lands in the inbox without a
 * toast). Mirrors llm-provider's LlmModelDownloadNotifications. The durable row
 * + badge update are handled by the Notifications store's own subscription.
 */
export function NotificationToastListener() {
  useEffect(() => {
    const GROUP = 'NotificationToastListener'
    Stores.EventBus.on(
      'sync:notification',
      async event => {
        if (event.data.action !== 'create') return
        if (!hasPermissionNow(Permissions.NotificationsRead)) return
        const id = event.data.id
        // Nil id = a bulk "list changed" signal (read-all / prune), not a new row.
        if (!id || id === '00000000-0000-0000-0000-000000000000') return
        try {
          const n = await ApiClient.Notification.get({ id })
          if (!n.interrupt) return
          if (n.kind === 'scheduled_task_failed') {
            message.error(n.title, { description: n.body || undefined })
          } else {
            message.info(n.title, { description: n.body || undefined })
          }
        } catch {
          /* best-effort toast */
        }
      },
      GROUP,
    )
    return () => {
      Stores.EventBus.removeGroupListeners(GROUP)
    }
  }, [])
  return null
}
