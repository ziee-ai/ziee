import { SPLIT_LIMITS } from '@/modules/chat/core/split/limits'
import type { SplitViewSet, SplitViewGet } from '../state'
import newPaneId from './_newPaneId'

export default (set: SplitViewSet, get: SplitViewGet) => {
  return async (opts?: {
    conversationId?: string | null
    projectId?: string | null
    afterPaneId?: string
    /** Insert the new pane immediately BEFORE this pane (ITEM-70 edge-drop
     *  insert-left). Ignored if `afterPaneId` is also set. */
    beforePaneId?: string
  }) => {
    // One conversation per workspace (ITEM-24): opening a conversation already
    // in a pane focuses that pane instead of creating a duplicate.
    if (opts?.conversationId) {
      const existing = get().panes.find(
        (p) => p.conversationId === opts.conversationId,
      )
      if (existing) {
        set((d) => {
          d.focusedPaneId = existing.paneId
        })
        return existing.paneId
      }
    }
    if (get().panes.length >= SPLIT_LIMITS.MAX_PANES) return null
    const paneId = newPaneId()
    const pane: import('../state').Pane = {
      paneId,
      conversationId: opts?.conversationId ?? null,
      projectId: opts?.projectId ?? null,
    }
    set((d) => {
      if (opts?.afterPaneId) {
        const idx = d.panes.findIndex((p) => p.paneId === opts.afterPaneId)
        if (idx >= 0) d.panes.splice(idx + 1, 0, pane)
        else d.panes.push(pane)
      } else if (opts?.beforePaneId) {
        const idx = d.panes.findIndex((p) => p.paneId === opts.beforePaneId)
        if (idx >= 0) d.panes.splice(idx, 0, pane)
        else d.panes.push(pane)
      } else {
        d.panes.push(pane)
      }
      d.focusedPaneId = paneId
    })
    return paneId
  }
}
