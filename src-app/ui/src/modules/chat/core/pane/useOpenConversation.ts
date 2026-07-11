import { useCallback } from 'react'
import { useNavigate } from 'react-router-dom'
import { dialog } from '@/components/ui'
import { Stores } from '@/core'
import { SPLIT_LIMITS } from '@/modules/chat/core/split/limits'
import {
  needsOpenChoice,
  type ReconcileIntent,
} from '@/modules/chat/core/split/reconcile'

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
    async (
      conversationId: string,
      opts?: {
        intent?: ReconcileIntent
        projectId?: string | null
        /** Override the destination URL (e.g. a project-scoped chat href). */
        href?: string
      },
    ) => {
      let intent = opts?.intent ?? 'auto'
      const sv = Stores.SplitView.$

      // FB-8 / ITEM-43: an AMBIGUOUS plain open (a split is open and this
      // conversation isn't already in a pane) asks the user how to place it,
      // instead of silently replacing the focused pane. `needsOpenChoice` is the
      // pure trigger (DEC-58); single-pane opens, already-open focuses, and
      // explicit Cmd/menu intents skip this entirely.
      if (
        needsOpenChoice(
          { panes: sv.panes, focusedPaneId: sv.focusedPaneId },
          conversationId,
          intent,
        )
      ) {
        const atCap = sv.panes.length >= SPLIT_LIMITS.MAX_PANES
        const choice = await dialog.choose({
          title: 'Open this conversation',
          description: 'A split view is open — how should this conversation open?',
          options: [
            { key: 'single', label: 'Open as single pane' },
            { key: 'replace', label: 'Replace the active pane' },
            // Only offer a new pane when there's room (else it would hit the cap).
            ...(atCap ? [] : [{ key: 'new', label: 'Add as a new pane' }]),
          ],
          cancelText: 'Cancel',
          testid: 'open-conversation-choice',
        })
        if (!choice) return // cancelled / dismissed
        if (choice === 'single') {
          Stores.SplitView.reset() // collapse to single-pane (URL-driven)
          navigate(opts?.href ?? `/chat/${conversationId}`)
          return
        }
        intent = choice === 'replace' ? 'replaceFocused' : 'newPane'
      }

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
        // MAX_PANES reached (ITEM-29): offer to REPLACE the focused pane instead
        // of silently no-op-ing, so the click still does something useful.
        const replace = await dialog.confirm({
          title: `You already have ${SPLIT_LIMITS.MAX_PANES} conversations open`,
          description:
            'Replace the focused pane with this conversation instead?',
          okText: 'Replace focused pane',
          cancelText: 'Cancel',
        })
        if (!replace) return
        const replaced = Stores.SplitView.openConversationInWorkspace(
          conversationId,
          'replaceFocused',
          { currentConversationId, projectId: opts?.projectId ?? null },
        )
        if (replaced.kind === 'capReached') return
        navigate(opts?.href ?? `/chat/${conversationId}`)
        return
      }
      navigate(opts?.href ?? `/chat/${conversationId}`)
    },
    [navigate],
  )
}

/**
 * Close a split pane, exiting to single-pane view when only one pane remains
 * (ITEM-29 "close-to-1"). The workspace stays the layout source of truth while
 * ≥2 panes are open; dropping to one collapses the workspace (single-pane is
 * URL-driven) and navigates to the survivor so the URL and the shown
 * conversation stay in lockstep.
 */
export function useClosePane() {
  const navigate = useNavigate()
  return useCallback(
    (paneId: string) => {
      const survivors = Stores.SplitView.$.panes.filter(
        (p) => p.paneId !== paneId,
      )
      Stores.SplitView.closePane(paneId)
      if (survivors.length === 1) {
        const only = survivors[0]
        Stores.SplitView.reset() // collapse to single-pane (URL-driven)
        navigate(only.conversationId ? `/chat/${only.conversationId}` : '/chat')
      }
    },
    [navigate],
  )
}
