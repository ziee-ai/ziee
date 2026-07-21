import type { SplitViewSet, SplitViewGet } from '../state'

export default (set: SplitViewSet, _get: SplitViewGet) => {
  return async (fromIndex: number, toIndex: number) => {
    set((d) => {
      const n = d.panes.length
      if (fromIndex < 0 || fromIndex >= n || toIndex < 0 || toIndex >= n) return
      const [moved] = d.panes.splice(fromIndex, 1)
      d.panes.splice(toIndex, 0, moved)
    })
  }
}
