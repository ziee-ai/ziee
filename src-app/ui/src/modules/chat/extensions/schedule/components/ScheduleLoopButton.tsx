import { useState } from 'react'
import { Permissions } from '@/api-client/permissions'
import { CalendarClock } from 'lucide-react'
import { Button, Tooltip } from '@ziee/kit'

import { usePermission } from '@/core/permissions'

import { useChatPaneOrNull } from '@/modules/chat/core/pane/ChatPaneContext'

import { ScheduleLoopDialog } from './ScheduleLoopDialog'
import { Chat } from '@/modules/chat/core/stores/chatBridge'

/**
 * Composer affordance (Group E, ITEM-18 / DEC-41) — a single toolbar button that
 * opens the merged "Schedule or loop this chat" dialog, mirroring the voice
 * `MicButton` (same `toolbar_actions` slot, same ghost icon-button + Tooltip).
 *
 * Gating:
 *   - HIDDEN entirely when the user lacks `scheduler::use` (permission layer 4).
 *   - DISABLED (with an explanatory tooltip) until the chat has a saved
 *     conversation, since a schedule/loop must bind to a real, owned conversation
 *     (`bound_conversation_id`; the backend 404s a foreign/absent id).
 *
 * Binds to THIS pane's chat store (not the focused-pane bridge) so a split pane
 * schedules ITS own conversation — same pattern as `MicButton`.
 */
export function ScheduleLoopButton() {
  const pane = useChatPaneOrNull()
  const chat = (pane?.store ?? Chat) as typeof Chat
  const { conversation } = chat
  const canUse = usePermission(Permissions.SchedulerUse)
  const [open, setOpen] = useState(false)

  // Permission gate first: no `scheduler::use` → no affordance at all.
  if (!canUse) return null

  const conversationId = conversation?.id ?? null
  const disabled = !conversationId

  return (
    <>
      <Tooltip
        content={
          disabled
            ? 'Send a message first, then schedule or loop this chat'
            : 'Schedule or loop this chat'
        }
      >
        <span className="inline-flex shrink-0">
          <Button
            data-testid="chat-schedule-loop-button"
            data-tooltip-wrapped=""
            icon={<CalendarClock className="size-4" />}
            variant="ghost"
            size="default"
            aria-label="Schedule or loop this chat"
            disabled={disabled}
            onClick={() => setOpen(true)}
          />
        </span>
      </Tooltip>
      {conversationId && (
        <ScheduleLoopDialog
          open={open}
          onClose={() => setOpen(false)}
          conversationId={conversationId}
          defaultModelId={conversation?.model_id ?? ''}
        />
      )}
    </>
  )
}
