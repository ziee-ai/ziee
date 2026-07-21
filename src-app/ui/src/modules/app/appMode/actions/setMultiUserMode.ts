import type { AppModeGet, AppModeSet } from '../state'

export default (set: AppModeSet, _get: AppModeGet) =>
  async (value: boolean) => {
    set({ multiUserMode: value })
  }
