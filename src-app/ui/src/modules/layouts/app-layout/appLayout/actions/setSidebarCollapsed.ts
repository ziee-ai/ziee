import type { AppLayoutGet, AppLayoutSet } from '../state'

export default (set: AppLayoutSet, _get: AppLayoutGet) =>
  async (isSidebarCollapsed: boolean) => {
    set({ isSidebarCollapsed })
  }
