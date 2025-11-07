import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import type { LlmProvider } from '@/api-client/types'
import { ApiClient } from '@/api-client'
import { Stores } from '@/core/stores'

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

  // Cached providers
  allProviders: LlmProvider[]
  providersLoading: boolean
  providersError: string | null
  providersInitialized: boolean

  // Initialization
  __init__: {
    __store__: () => void
    allProviders: () => Promise<void>
  }

  // Actions
  loadAllProviders: () => Promise<void>
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
        allProviders: [],
        providersLoading: false,
        providersError: null,
        providersInitialized: false,
        __init__: {
          // Store-level initialization - runs once on first access (any property)
          __store__: () => {
            // Subscribe to group provider assignment changes
            const eventBus = Stores.EventBus

            // When providers are assigned to a group, update the cache directly
            eventBus.on('llm_provider.group_providers_changed', async event => {
              const { groupId, providerIds } = event.data

              // Ensure providers are loaded
              await get().loadAllProviders()

              // Use cached providers to build the assigned list
              const allProviders = get().allProviders
              const assignedProviders = allProviders.filter(p => providerIds.includes(p.id))

              set(state => {
                state.groupProviders.set(groupId, {
                  groupId,
                  providers: assignedProviders,
                  loading: false,
                  error: null,
                  lastFetched: Date.now(),
                })
              })
            })
          },

          // Property-specific initialization - runs when allProviders is first accessed
          allProviders: async () => {
            await get().loadAllProviders()
          },
        },

        /**
         * Load all providers (cached)
         * Only fetches if not already initialized
         */
        loadAllProviders: async (): Promise<void> => {
          const state = get()

          // If already loading, don't start another fetch
          if (state.providersLoading) {
            return
          }

          // If already initialized, use cached data
          if (state.providersInitialized && !state.providersError) {
            return
          }

          set(state => {
            state.providersLoading = true
            state.providersError = null
          })

          try {
            const response = await ApiClient.LlmProvider.list({ page: 1, per_page: 1000 })

            set(state => {
              state.allProviders = response.providers
              state.providersLoading = false
              state.providersError = null
              state.providersInitialized = true
            })
          } catch (error) {
            console.error('Failed to load providers:', error)

            set(state => {
              state.providersLoading = false
              state.providersError = error instanceof Error ? error.message : 'Failed to load providers'
            })

            throw error
          }
        },

        /**
         * Load providers for a specific group
         * Uses cached providers instead of fetching every time
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
