import type { SplitViewSet, SplitViewGet } from '../state'

export default (set: SplitViewSet, get: SplitViewGet) => {
  return async (
    paneId: string,
    conversationId: string | null,
    projectId: string | null = null,
  ) => {
    // One conversation per workspace (ITEM-24): pointing a pane at a
    // conversation already open in a DIFFERENT pane focuses that pane rather
    // than creating a duplicate. (Adopting a brand-new conversation into its
    // own new-chat pane is unaffected — the conversation is in no other pane.)
    if (conversationId) {
      const dup = get().panes.find(
        (p) => p.conversationId === conversationId && p.paneId !== paneId,
      )
      if (dup) {
        set((d) => {
          d.focusedPaneId = dup.paneId
        })
        return
      }
    }
    set((d) => {
      const p = d.panes.find((pp) => pp.paneId === paneId)
      if (p) {
        p.conversationId = conversationId
        p.projectId = projectId
      }
    })
  }
}
