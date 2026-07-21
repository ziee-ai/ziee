import { ApiClient } from '@/api-client'
import type { TunnelAuthGet, TunnelAuthSet } from '../state'

export default (set: TunnelAuthSet, get: TunnelAuthGet) =>
  async () => {
    if (get().loadingConfig) return
    set({ loadingConfig: true, configError: null })
    try {
      const cfg = await ApiClient.Auth.getConfig(undefined, undefined)
      set({ authConfig: cfg, loadingConfig: false })
    } catch (e) {
      set({
        loadingConfig: false,
        configError: e instanceof Error ? e.message : 'Failed to load login config',
      })
    }
  }
