import type { SplitViewGet, SplitViewSet } from '../state'
import {
  openConversationInWorkspace as reconcileOpen,
  type ReconcileIntent,
  type ReconcileOutcome,
} from '@/modules/chat/core/split/reconcile'
import newPaneId from './_newPaneId'

export default (set: SplitViewSet, get: SplitViewGet) => {
  /**
   * The v2 workspace entry point (ITEM-24/25) — route EVERY "open conversation"
   * request (sidebar click, ⋯-menu, drag-drop, the router URL effect) through
   * the pure reconciliation reducer so the workspace behaves identically no
   * matter how the conversation was opened, and never duplicates a conversation
   * across panes. Applies the reducer's `next` layout and returns its `outcome`
   * so the caller can do the impure follow-up (navigate / toast / offer-replace).
   */
  return async (
    conversationId: string,
    intent: ReconcileIntent,
    opts?: { currentConversationId?: string | null; projectId?: string | null },
  ): Promise<ReconcileOutcome> => {
    const { next, outcome } = reconcileOpen({
      layout: {
        panes: get().panes,
        focusedPaneId: get().focusedPaneId,
      },
      currentConversationId: opts?.currentConversationId ?? null,
      conversationId,
      projectId: opts?.projectId ?? null,
      intent,
      newPaneId,
    })
    set((d) => {
      d.panes = next.panes
      d.focusedPaneId = next.focusedPaneId
      // Drop any divider widths beyond the surviving pane-gap count.
      if (d.dividerWidths.length > Math.max(0, d.panes.length - 1)) {
        d.dividerWidths.length = Math.max(0, d.panes.length - 1)
      }
    })
    return outcome
  }
}
