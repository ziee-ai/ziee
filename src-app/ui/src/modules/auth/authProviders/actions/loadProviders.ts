import { ApiClient } from '@/api-client'
import type { AuthProvidersGet, AuthProvidersSet } from '../state'

export default (set: AuthProvidersSet, get: AuthProvidersGet) =>
  async () => {
    if (get().isLoading) return
    set({ isLoading: true, error: null })
    try {
      const res = await ApiClient.Auth.listProviders(undefined, undefined)
      set({ providers: res.providers, isLoading: false, hasLoaded: true })
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to load providers',
        isLoading: false,
        hasLoaded: true,
      })
    }
  }
