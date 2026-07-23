import { useEffect } from 'react'
import { useLocation, useNavigate } from 'react-router-dom'
import { EventBus } from '@ziee/framework/stores'
import { SplitView } from '@/modules/chat/core/stores/splitView'

/**
 * Leave `/chat/:id` when THAT conversation is deleted (issue #168).
 *
 * Deleting the conversation you are currently viewing dropped its sidebar row but
 * left the main content pane rendering the gone conversation, with the URL still on
 * its dead id. Nothing owned that transition: `ChatHistory.deleteConversation`
 * deliberately doesn't route (a store shouldn't), none of its call sites navigate,
 * `SplitView.closePaneForConversation` is a no-op in single-pane mode (`panes` is
 * empty), and `ConversationPage`'s workspace→URL effect bails at `panes.length === 0`
 * because a pure single-pane view is URL-driven. This hook closes that gap.
 *
 * Keeps the pure-store / impure-navigation split (`core/split/reconcile.ts` returns an
 * outcome, the caller navigates) — it's the sibling of `useClosePane()`, owning
 * `useNavigate` at the route level rather than pushing routing into a store.
 *
 * Subscribes to BOTH delete origins, mirroring `SplitView.store.ts:362-372`:
 * `conversation.deleted` (this device's own delete; the SSE self-echo is suppressed
 * so the sync handler never fires for it) and `sync:conversation` with
 * `action: 'delete'` (cross-device).
 */
export function useNavigateAwayOnDelete(conversationId: string | undefined) {
  const navigate = useNavigate()
  // `PopoutConversationPage` renders this same `ConversationPage` under
  // `/chat-window/:id` — a layout-less pop-out window. Navigating THAT window to
  // `/chat` would drop the full app shell into a chrome-less pop-out, so leave it
  // to the pop-out's own lifecycle (`usePopoutSnapBack`) and only steer the main
  // window here.
  const isPopout = useLocation().pathname.startsWith('/chat-window/')

  useEffect(() => {
    if (!conversationId || isPopout) return

    const handleDeleted = (deletedId: string) => {
      // Deleting a conversation we're NOT showing must leave the active pane
      // alone — only its sidebar row goes.
      if (deletedId !== conversationId) return

      // A split workspace has its own recovery: SplitView closes the pane holding
      // the deleted conversation and `ConversationPage`'s workspace→URL effect
      // (FB-25) repoints the URL at the surviving focused pane. Bail so we don't
      // fight it and bounce the user to the start page instead of the survivor.
      //
      // Filter the deleted id out ourselves rather than reading `panes.length`:
      // SplitView subscribes at store init and this hook at mount, so which
      // handler runs first is not contractual. Filtering makes the decision
      // order-independent.
      const survivors = SplitView.$.panes.filter(
        (p) => p.conversationId !== deletedId,
      )
      if (survivors.length > 0) return

      // No pane survives (pure single-pane, or a collapsed 1-pane workspace whose
      // only conversation this was) → collapse the now-empty workspace and go to
      // the start / new-chat page. Same close-to-1 shape as `useClosePane()`.
      // `replace` so Back doesn't land on the dead id.
      SplitView.reset()
      navigate('/chat', { replace: true })
    }

    const offLocal = EventBus.on(
      'conversation.deleted',
      (event) => handleDeleted(event.data.conversationId),
      'useNavigateAwayOnDelete',
    )
    const offSync = EventBus.on(
      'sync:conversation',
      (event) => {
        if (event.data.action !== 'delete') return
        handleDeleted(event.data.id)
      },
      'useNavigateAwayOnDelete',
    )

    return () => {
      offLocal()
      offSync()
    }
  }, [conversationId, isPopout, navigate])
}
