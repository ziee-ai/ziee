import type { SplitDirection } from '@/modules/chat/core/split/limits'
import type { SplitViewSet, SplitViewGet } from '../state'

export default (set: SplitViewSet, _get: SplitViewGet) => {
  return async (w: {
    panes: import('../state').Pane[]
    focusedPaneId: string | null
    dividerWidths: number[]
    direction: SplitDirection
    mode: 'split' | 'tabs'
  }) => {
    set((d) => {
      d.panes = w.panes
      d.focusedPaneId = w.focusedPaneId
      d.dividerWidths = w.dividerWidths
      d.direction = w.direction
      d.mode = w.mode
    })
  }
}
