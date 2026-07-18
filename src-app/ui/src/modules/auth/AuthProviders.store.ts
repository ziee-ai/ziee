import { ApiClient } from '@/api-client'
import type { PublicProvider } from '@/api-client/types'
import { type StoreProxy } from '@ziee/framework/stores'
import { defineStore } from '@ziee/framework/store-kit'

interface AuthProvidersState {
  providers: PublicProvider[]
  isLoading: boolean
  error?: string | null
  hasLoaded: boolean
  loadProviders: () => Promise<void>
}

declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    AuthProviders: StoreProxy<AuthProvidersState>
  }
}

export const AuthProviders = defineStore('AuthProviders', {
  state: {
    providers: [] as PublicProvider[],
    isLoading: false,
    error: null as string | null | undefined,
    hasLoaded: false,
  },
  actions: (set, get) => ({
    loadProviders: async () => {
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
    },
  }),
  // Auto-load on first access (was `__init__.providers`) so the login page's
  // <ProviderButtons> doesn't fetch in a useEffect.
  init: ({ actions }) => {
    void actions.loadProviders()
  },
})

export const useAuthProvidersStore = AuthProviders.store
