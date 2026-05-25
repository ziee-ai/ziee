import { create } from 'zustand'
import { ApiClient } from '@/api-client'
import type { PublicProvider } from '@/api-client/types'
import { type StoreProxy } from '@/core/stores'

interface AuthProvidersState {
  providers: PublicProvider[]
  isLoading: boolean
  error?: string | null
  hasLoaded: boolean

  loadProviders: () => Promise<void>
}

declare module '../../core/stores' {
  interface RegisteredStores {
    AuthProviders: StoreProxy<AuthProvidersState>
  }
}

export const useAuthProvidersStore = create<AuthProvidersState>((set, get) => ({
  providers: [],
  isLoading: false,
  error: null,
  hasLoaded: false,

  loadProviders: async () => {
    if (get().isLoading) return
    set({ isLoading: true, error: null })
    try {
      const res = await ApiClient.Auth.listProviders(undefined, undefined)
      set({
        providers: res.providers,
        isLoading: false,
        hasLoaded: true,
      })
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to load providers',
        isLoading: false,
        hasLoaded: true,
      })
    }
  },
}))
