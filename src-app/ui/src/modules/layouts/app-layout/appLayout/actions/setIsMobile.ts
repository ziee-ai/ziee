import type { AppLayoutGet, AppLayoutSet } from '../state'

export default (set: AppLayoutSet, _get: AppLayoutGet) =>
  async (isMobile: boolean) => {
    set({ isMobile })
  }
