import { SPLIT_LIMITS } from '@/modules/chat/core/split/limits'
import type { SplitViewSet, SplitViewGet } from '../state'

export default (set: SplitViewSet, _get: SplitViewGet) => {
  return async (index: number, width: number) => {
    const w = Math.max(
      SPLIT_LIMITS.MIN_PANE_WIDTH,
      Math.min(SPLIT_LIMITS.MAX_PANE_WIDTH, Math.round(width)),
    )
    set((d) => {
      d.dividerWidths[index] = w
    })
  }
}
