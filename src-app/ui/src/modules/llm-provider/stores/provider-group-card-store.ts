import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import type { Group } from '@/api-client/types'
import { ApiClient } from '@/api-client'

interface ProviderGroups {
  providerId: string
  groups: Group[]
  loading: boolean
  error: string | null
  lastFetched: number | null
}

interface ProviderGroupCardState {
  // Map of providerId -> group data
  providerGroups: Map<string, ProviderGroups>

  // Initialization
  __init__: Record<string, never>

  // Actions
  loadGroupsForProvider: (providerId: string, force?: boolean) => Promise<void>
  clearProviderGroups: (providerId: string) => void
  clearAllProviderGroups: () => void
  getProviderGroupsData: (providerId: string) => ProviderGroups | undefined
}

export const useProviderGroupCardStore = create<ProviderGroupCardState>()(
  subscribeWithSelector(
    immer(
      (set, get): ProviderGroupCardState => ({
        providerGroups: new Map(),
        __init__: {},

        /**
         * Load groups for a specific provider
         * Caches results and prevents duplicate fetches
         */
        loadGroupsForProvider: async (providerId: string, force = false): Promise<void> => {
          const state = get()
          const existing = state.providerGroups.get(providerId)

          // If already loading, don't start another fetch
          if (existing?.loading && !force) {
            return
          }

          // If data is fresh (< 30 seconds old) and not forcing, use cached data
          if (
            !force &&
            existing?.lastFetched &&
            Date.now() - existing.lastFetched < 30000 &&
            !existing.error
          ) {
            return
          }

          // Set loading state
          set(state => {
            state.providerGroups.set(providerId, {
              providerId,
              groups: existing?.groups || [],
              loading: true,
              error: null,
              lastFetched: existing?.lastFetched || null,
            })
          })

          try {
            const groups = await ApiClient.LlmProvider.getGroups({ provider_id: providerId })

            set(state => {
              state.providerGroups.set(providerId, {
                providerId,
                groups,
                loading: false,
                error: null,
                lastFetched: Date.now(),
              })
            })
          } catch (error) {
            console.error(`Failed to load groups for provider ${providerId}:`, error)

            set(state => {
              state.providerGroups.set(providerId, {
                providerId,
                groups: existing?.groups || [],
                loading: false,
                error: error instanceof Error ? error.message : 'Failed to load groups',
                lastFetched: existing?.lastFetched || null,
              })
            })

            throw error
          }
        },

        /**
         * Clear cached data for a specific provider
         */
        clearProviderGroups: (providerId: string): void => {
          set(state => {
            state.providerGroups.delete(providerId)
          })
        },

        /**
         * Clear all cached group data
         */
        clearAllProviderGroups: (): void => {
          set(state => {
            state.providerGroups.clear()
          })
        },

        /**
         * Get groups for a specific provider from the store
         */
        getProviderGroupsData: (providerId: string): ProviderGroups | undefined => {
          return get().providerGroups.get(providerId)
        },
      }),
    ),
  ),
)
