import type { RuntimeModelUsageGet, RuntimeModelUsageSet } from '../state'

export default (set: RuntimeModelUsageSet, _get: RuntimeModelUsageGet) =>
  async () => {
    set({ error: null })
  }
