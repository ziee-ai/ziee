import type { AppLayoutGet, AppLayoutSet } from '../state'

export default (set: AppLayoutSet, _get: AppLayoutGet) =>
  async (nativeScroll: boolean) => {
    set({ nativeScroll })
  }
