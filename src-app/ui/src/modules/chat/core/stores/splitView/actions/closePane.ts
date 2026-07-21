import type { SplitViewSet, SplitViewGet } from '../state'

export default (set: SplitViewSet, _get: SplitViewGet) => {
  return async (paneId: string) => {
    set((d) => {
      const idx = d.panes.findIndex((p) => p.paneId === paneId)
      if (idx < 0) return
      d.panes.splice(idx, 1)
      if (d.dividerWidths.length > Math.max(0, d.panes.length - 1)) {
        d.dividerWidths.length = Math.max(0, d.panes.length - 1)
      }
      if (d.focusedPaneId === paneId) {
        const next = d.panes[idx] ?? d.panes[idx - 1] ?? d.panes[0] ?? null
        d.focusedPaneId = next ? next.paneId : null
      }
    })
  }
}
