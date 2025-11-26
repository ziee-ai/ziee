import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import type { Group } from '@/api-client/types'
import { ApiClient } from '@/api-client'
import { Stores } from '@/core/stores'

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

  // Cached user groups
  allGroups: Group[]
  groupsLoading: boolean
  groupsError: string | null
  groupsInitialized: boolean

  // Initialization
  __init__: {
    __store__: () => void
    allGroups: () => Promise<void>
  }

  // Actions
  loadAllGroups: () => Promise<void>
  loadGroupsForProvider: (providerId: string, force?: boolean) => Promise<void>
  clearProviderGroups: (providerId: string) => void
  clearAllProviderGroups: () => void
  getProviderGroupsData: (providerId: string) => ProviderGroups | undefined

  // Cleanup
  __destroy__?: () => void
}

export const useProviderGroupCardStore = create<ProviderGroupCardState>()(
  subscribeWithSelector(
    immer(
      (set, get): ProviderGroupCardState => ({
        providerGroups: new Map(),
        allGroups: [],
        groupsLoading: false,
        groupsError: null,
        groupsInitialized: false,
        __init__: {
          // Store-level initialization - runs once on first access (any property)
          __store__: () => {
            const GROUP = 'ProviderGroupAssignmentCardStore'
            // Subscribe to group events for cache invalidation
            const eventBus = Stores.EventBus

            // Common handler to invalidate cache and reload groups
            const handleGroupChange = () => {
              set({ groupsInitialized: false })
              get().loadAllGroups()
            }

            // Type-safe - TypeScript infers event types automatically
            eventBus.on('group.created', handleGroupChange, GROUP)
            eventBus.on('group.updated', handleGroupChange, GROUP)
            eventBus.on('group.deleted', handleGroupChange, GROUP)

            // Subscribe to provider group assignment changes
            // When groups are assigned to a provider, update the cache directly
            eventBus.on(
              'llm_provider.groups_changed',
              async event => {
                const { providerId, groupIds } = event.data
                await get().loadAllGroups()
                const allGroups = get().allGroups
                const assignedGroups = allGroups.filter(g =>
                  groupIds.includes(g.id),
                )

                set(state => {
                  state.providerGroups.set(providerId, {
                    providerId,
                    groups: assignedGroups,
                    loading: false,
                    error: null,
                    lastFetched: Date.now(),
                  })
                })
              },
              GROUP,
            )
          },

          // Property-specific initialization - runs when allGroups is first accessed
          allGroups: async () => {
            await get().loadAllGroups()
          },
        },

        /**
         * Load all user groups (cached)
         * Only fetches if not already initialized
         */
        loadAllGroups: async (): Promise<void> => {
          const state = get()

          // If already loading, don't start another fetch
          if (state.groupsLoading) {
            return
          }

          // If already initialized, use cached data
          if (state.groupsInitialized && !state.groupsError) {
            return
          }

          set(state => {
            state.groupsLoading = true
            state.groupsError = null
          })

          try {
            const response = await ApiClient.UserGroup.list({
              page: 1,
              per_page: 1000,
            })

            set(state => {
              state.allGroups = response.groups
              state.groupsLoading = false
              state.groupsError = null
              state.groupsInitialized = true
            })
          } catch (error) {
            console.error('Failed to load user groups:', error)

            set(state => {
              state.groupsLoading = false
              state.groupsError =
                error instanceof Error ? error.message : 'Failed to load groups'
            })

            throw error
          }
        },

        /**
         * Load groups for a specific provider
         * Uses cached groups instead of fetching every time
         */
        loadGroupsForProvider: async (
          providerId: string,
          force = false,
        ): Promise<void> => {
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
            // Get groups for this provider (API returns full Group objects)
            const groups = await ApiClient.LlmProvider.getGroups({
              provider_id: providerId,
            })

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
            console.error(
              `Failed to load groups for provider ${providerId}:`,
              error,
            )

            set(state => {
              state.providerGroups.set(providerId, {
                providerId,
                groups: existing?.groups || [],
                loading: false,
                error:
                  error instanceof Error
                    ? error.message
                    : 'Failed to load groups',
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
        getProviderGroupsData: (
          providerId: string,
        ): ProviderGroups | undefined => {
          return get().providerGroups.get(providerId)
        },

        /**
         * Cleanup method - removes all event listeners for this store
         */
        __destroy__: () => {
          Stores.EventBus.removeGroupListeners(
            'ProviderGroupAssignmentCardStore',
          )
        },
      }),
    ),
  ),
)
