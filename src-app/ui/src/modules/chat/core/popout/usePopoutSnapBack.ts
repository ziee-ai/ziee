import { useEffect } from 'react'
import { Stores } from '@/core'
import { SPLIT_LIMITS } from '@/modules/chat/core/split/limits'
import {
  registerPopoutCloseEmitter,
  registerMainWindowSnapBackListener,
} from './popoutSnapBack'

/** Parse a conversation id out of a `/chat/<id>` path (the main window's URL). */
function conversationIdFromPath(pathname: string): string | null {
  const m = pathname.match(/\/chat\/([^/?#]+)/)
  return m ? m[1] : null
}

/**
 * Mounted in a POP-OUT window (the `/chat-window/:id` route): registers the desktop
 * close-emitter so closing this window snaps its conversation back into the main
 * window (ITEM-54). Web / non-conversation: no-op (the seam's web base returns inert).
 */
export function usePopoutCloseEmitter(conversationId: string | undefined): void {
  useEffect(() => {
    if (!conversationId) return
    let un: (() => void) | undefined
    let cancelled = false
    void registerPopoutCloseEmitter(conversationId).then(u => {
      if (cancelled) u()
      else un = u
    })
    return () => {
      cancelled = true
      un?.()
    }
  }, [conversationId])
}

/**
 * Mounted ONCE in the MAIN window (via `AppLayout`, which the layout-less pop-out
 * route does NOT render): listens for a pop-out window closing and snaps its
 * conversation back into the workspace as a pane (ITEM-54). Web: no-op.
 */
export function usePopoutSnapBackListener(): void {
  useEffect(() => {
    let un: (() => void) | undefined
    let cancelled = false
    void registerMainWindowSnapBackListener({
      getPaneConversationIds: () =>
        Stores.SplitView.$.panes.map(p => p.conversationId),
      getSinglePaneConversationId: () =>
        conversationIdFromPath(window.location.pathname),
      maxPanes: SPLIT_LIMITS.MAX_PANES,
      openAsNewPane: id => {
        const sv = Stores.SplitView.$
        const focused = sv.panes.find(p => p.paneId === sv.focusedPaneId)
        const currentConversationId =
          focused?.conversationId ??
          conversationIdFromPath(window.location.pathname)
        Stores.SplitView.openConversationInWorkspace(id, 'newPane', {
          currentConversationId,
          projectId: null,
        })
      },
    }).then(u => {
      if (cancelled) u()
      else un = u
    })
    return () => {
      cancelled = true
      un?.()
    }
  }, [])
}
