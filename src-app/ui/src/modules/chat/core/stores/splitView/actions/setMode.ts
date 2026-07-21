import type { SplitViewSet, SplitViewGet } from '../state'

export default (set: SplitViewSet, _get: SplitViewGet) => {
  return async (mode: 'split' | 'tabs') => {
    set((d) => {
      d.mode = mode
    })
  }
}
