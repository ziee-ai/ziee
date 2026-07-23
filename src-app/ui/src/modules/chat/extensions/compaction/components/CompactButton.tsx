import { useState } from 'react'
import { Permissions } from '@/api-client/permissions'
import { FoldVertical } from 'lucide-react'
import { Button, Tooltip, message } from '@ziee/kit'

import { ApiClient } from '@/api-client'
import { usePermission } from '@/core/permissions'

import { useChatPaneOrNull } from '@/modules/chat/core/pane/ChatPaneContext'
import { Chat } from '@/modules/chat/core/stores/chatBridge'

/**
 * Composer affordance (ITEM-61 / DEC-137) — a toolbar button that manually COMPACTS
 * this conversation's context (`POST /conversations/{id}/compact`), so a long thread
 * can be summarized on demand instead of only automatically. Mirrors the
 * `ScheduleLoopButton` (same `toolbar_actions` slot, ghost icon-button + Tooltip).
 *
 * Gating:
 *   - HIDDEN when the user lacks `conversations::edit` (the endpoint's perm).
 *   - DISABLED until the chat has a saved conversation (nothing to compact yet).
 *
 * Binds to THIS pane's chat store so a split pane compacts ITS own conversation.
 */
export function CompactButton() {
  const pane = useChatPaneOrNull()
  const chat = (pane?.store ?? Chat) as typeof Chat
  const { conversation } = chat
  const canEdit = usePermission(Permissions.ConversationsEdit)
  const [busy, setBusy] = useState(false)

  if (!canEdit) return null

  const conversationId = conversation?.id ?? null
  const disabled = !conversationId || busy

  const onCompact = async () => {
    if (!conversationId) return
    setBusy(true)
    try {
      await ApiClient.Conversation.compact({ id: conversationId })
      message.success('Context compacted')
    } catch (e) {
      message.error(e instanceof Error ? e.message : 'Failed to compact')
    } finally {
      setBusy(false)
    }
  }

  return (
    <Tooltip
      content={
        disabled && !busy
          ? 'Send a message first, then compact this chat'
          : 'Compact this conversation’s context'
      }
    >
      <span className="inline-flex shrink-0">
        <Button
          data-testid="chat-compact-button"
          data-tooltip-wrapped=""
          icon={<FoldVertical className="size-4" />}
          variant="ghost"
          size="default"
          aria-label="Compact this conversation’s context"
          loading={busy}
          disabled={disabled}
          onClick={onCompact}
        />
      </span>
    </Tooltip>
  )
}
