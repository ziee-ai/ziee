import type { SplitViewSet, SplitViewGet } from '../state'

export default (set: SplitViewSet, _get: SplitViewGet) => {
  return async () => {
    set((d) => {
      d.panes = []
      d.focusedPaneId = null
      d.dividerWidths = []
      d.mode = 'split'
    })
  }
}
