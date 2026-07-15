import type { ReactNode } from 'react'

import { Text } from '@ziee/kit'
import {
  type AppNotification,
  getNotificationRenderer,
  type NotificationRendererCtx,
  registerNotificationKind,
} from '@ziee/framework/notification'

import type { Notification } from '@/api-client/types'

/**
 * ziee's consumption of the SDK `@ziee/framework/notification` per-module RENDER
 * seam (the FE half of the backend `NOTIFICATION_KINDS` registry). ziee's
 * scheduler-produced kinds render as the standard title/body/timestamp block —
 * no kind-specific UI — so this shared renderer IS the generic content, and
 * registering it declares ziee's kinds to the render registry. Other modules /
 * apps can register richer renderers (custom content + an inline `actions` row)
 * for their own kinds; unregistered kinds fall back to the same block.
 */
function schedulerContent(n: AppNotification): ReactNode {
  return (
    <>
      <Text className="font-medium">{n.title}</Text>
      {n.body ? (
        <Text className="text-muted-foreground text-sm">{n.body}</Text>
      ) : null}
      <Text className="text-muted-foreground text-xs">
        {new Date(n.created_at).toLocaleString()}
      </Text>
    </>
  )
}

const schedulerRenderer = { render: schedulerContent }

// Side-effect registration (mirrors the backend `#[distributed_slice(
// NOTIFICATION_KINDS)]` declarations). Imported for effect by `module.tsx`.
registerNotificationKind('scheduled_task_result', schedulerRenderer)
registerNotificationKind('scheduled_task_failed', schedulerRenderer)

/**
 * Shell helper: render a notification's inbox content by dispatching on its
 * `kind` through the seam, falling back to the standard block for any
 * unregistered kind. Encapsulates the generated-`Notification` → seam
 * `AppNotification` shape bridge (structurally identical rows) in one place so
 * callers don't repeat the cast.
 */
export function renderNotificationContent(
  n: Notification,
  ctx: NotificationRendererCtx,
): ReactNode {
  const an = n as unknown as AppNotification
  const renderer = getNotificationRenderer(n.kind)
  return renderer ? renderer.render(an, ctx) : schedulerContent(an)
}
