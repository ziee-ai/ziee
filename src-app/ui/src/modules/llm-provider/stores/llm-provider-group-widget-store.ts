import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import type { LlmProvider } from '@/api-client/types'
import { ApiClient } from '@/api-client'

interface GroupProviders {
  groupId: string
  providers: LlmProvider[]
  loading: boolean
  error: string | null
  lastFetched: number | null
}

interface LlmProviderGroupWidgetState {
  // Map of groupId -> provider data
  groupProviders: Map<string, GroupProviders>

  // Initialization
  __init__: Record<string, never>

  // Actions
  loadProvidersForGroup: (groupId: string, force?: boolean) => Promise<void>
  clearGroupProviders: (groupId: string) => void
  clearAllGroupProviders: () => void
  getGroupProvidersData: (groupId: string) => GroupProviders | undefined
}

export const useLlmProviderGroupWidgetStore = create<LlmProviderGroupWidgetState>()(
  subscribeWithSelector(
    immer(
      (set, get): LlmProviderGroupWidgetState => ({
        groupProviders: new Map(),
        __init__: {},

        /**
         * Load providers for a specific group
         * Caches results and prevents duplicate fetches
         */
        loadProvidersForGroup: async (groupId: string, force = false): Promise<void> => {
          const state = get()
          const existing = state.groupProviders.get(groupId)

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
            state.groupProviders.set(groupId, {
              groupId,
              providers: existing?.providers || [],
              loading: true,
              error: null,
              lastFetched: existing?.lastFetched || null,
            })
          })

          try {
            const response = await ApiClient.Group.getProviders({ group_id: groupId })

            set(state => {
              state.groupProviders.set(groupId, {
                groupId,
                providers: response.providers,
                loading: false,
                error: null,
                lastFetched: Date.now(),
              })
            })
          } catch (error) {
            console.error(`Failed to load providers for group ${groupId}:`, error)

            set(state => {
              state.groupProviders.set(groupId, {
                groupId,
                providers: existing?.providers || [],
                loading: false,
                error: error instanceof Error ? error.message : 'Failed to load providers',
                lastFetched: existing?.lastFetched || null,
              })
            })

            throw error
          }
        },

        /**
         * Clear cached data for a specific group
         */
        clearGroupProviders: (groupId: string): void => {
          set(state => {
            state.groupProviders.delete(groupId)
          })
        },

        /**
         * Clear all cached provider data
         */
        clearAllGroupProviders: (): void => {
          set(state => {
            state.groupProviders.clear()
          })
        },

        /**
         * Get providers for a specific group from the store
         */
        getGroupProvidersData: (groupId: string): GroupProviders | undefined => {
          return get().groupProviders.get(groupId)
        },
      }),
    ),
  ),
)
