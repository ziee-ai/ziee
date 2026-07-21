import type { AppLayoutGet, AppLayoutSet } from '../state'

export default (set: AppLayoutSet, get: AppLayoutGet) =>
  async () => {
    set({ isSidebarCollapsed: !get().isSidebarCollapsed })
  }
