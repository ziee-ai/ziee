import { useCallback } from 'react'
import { useNavigate } from 'react-router-dom'
import { message } from '@/components/ui'
import { Stores } from '@/core'
import { SPLIT_LIMITS } from '@/modules/chat/core/split/limits'
import type { ReconcileIntent } from '@/modules/chat/core/split/reconcile'

/** Parse the conversation id out of a `/chat/<id>` (or `/projects/.../chat/<id>`) path. */
function conversationIdFromPath(pathname: string): string | null {
  const m = pathname.match(/\/chat\/([^/?#]+)/)
  return m ? m[1] : null
}

/**
 * The single sidebar/list open-conversation entry point (ITEM-28) — routes EVERY
 * "open this conversation" click through the workspace reconciliation reducer
 * (`SplitView.openConversationInWorkspace`, ITEM-25) instead of a raw `navigate`,
 * so the same click behaves identically whether the workspace is single-pane or
 * split, and never duplicates a conversation:
 *
 * - `intent: 'auto'` (plain click) → single-pane navigate, or replace the focused
 *   pane while a split is open, or focus the pane already holding it.
 * - `intent: 'newPane'` (Cmd/Ctrl/middle-click, the "Open in split pane" menu
 *   item) → open it beside the current pane (seeding `[current | X]` from
 *   single-pane); a MAX_PANES over-cap toasts instead of silently no-op-ing.
 *
 * The URL always tracks the focused pane, so a successful open navigates to the
 * conversation; `ConversationPage`'s loop-guarded URL effect (ITEM-25) reconciles
 * the reverse direction (deep links / back-forward) without a navigate↔focus loop.
 */
export function useOpenConversationInWorkspace() {
  const navigate = useNavigate()
  return useCallback(
    (
      conversationId: string,
      opts?: {
        intent?: ReconcileIntent
        projectId?: string | null
        /** Override the destination URL (e.g. a project-scoped chat href). */
        href?: string
      },
    ) => {
      const intent = opts?.intent ?? 'auto'
      const sv = Stores.SplitView.$
      const focused = sv.panes.find((p) => p.paneId === sv.focusedPaneId)
      // The single-pane base for a `newPane` bootstrap: the focused pane's
      // conversation, else whatever the URL currently shows.
      const currentConversationId =
        focused?.conversationId ??
        conversationIdFromPath(window.location.pathname)

      const outcome = Stores.SplitView.openConversationInWorkspace(
        conversationId,
        intent,
        { currentConversationId, projectId: opts?.projectId ?? null },
      )

      if (outcome.kind === 'capReached') {
        message.warning(
          `You can open at most ${SPLIT_LIMITS.MAX_PANES} conversations side by side. Close a pane first.`,
        )
        return
      }
      navigate(opts?.href ?? `/chat/${conversationId}`)
    },
    [navigate],
  )
}
