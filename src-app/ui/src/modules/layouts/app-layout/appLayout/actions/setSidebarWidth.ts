import type { AppLayoutGet, AppLayoutSet } from '../state'

export default (set: AppLayoutSet, _get: AppLayoutGet) =>
  async (sidebarWidth: number) => {
    set({ sidebarWidth })
  }
