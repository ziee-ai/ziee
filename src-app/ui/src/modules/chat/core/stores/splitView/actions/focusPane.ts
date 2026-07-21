import type { SplitViewSet, SplitViewGet } from '../state'

export default (set: SplitViewSet, _get: SplitViewGet) => {
  return async (paneId: string) => {
    set((d) => {
      if (d.panes.some((p) => p.paneId === paneId)) d.focusedPaneId = paneId
    })
  }
}
