import type { SplitViewSet, SplitViewGet } from '../state'

export default (set: SplitViewSet, _get: SplitViewGet) => {
  return async (open: boolean) => {
    set((d) => {
      d.paneManagerOpen = open
    })
  }
}
