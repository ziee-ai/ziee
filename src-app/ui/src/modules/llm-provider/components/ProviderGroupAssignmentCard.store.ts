import { Permissions, type Group } from '@/api-client/types'
import { ApiClient } from '@/api-client'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@/core/store-kit'

interface ProviderGroups {
  providerId: string
  groups: Group[]
  loading: boolean
  error: string | null
  lastFetched: number | null
}

export const ProviderGroupCard = defineStore('ProviderGroupCard', {
  immer: true,
  state: {
    // Map of providerId -> group data
    providerGroups: new Map<string, ProviderGroups>(),
    // Cached user groups
    allGroups: [] as Group[],
    groupsLoading: false,
    groupsError: null as string | null,
    groupsInitialized: false,
  },
  actions: (set, get) => {
    // Load all user groups (cached). Only fetches if not already initialized.
    const loadAllGroups = async (): Promise<void> => {
      const state = get()
      if (state.groupsLoading) return
      if (state.groupsInitialized && !state.groupsError) return
      set(s => {
        s.groupsLoading = true
        s.groupsError = null
      })
      try {
        const response = await ApiClient.UserGroup.list({ page: 1, per_page: 1000 })
        set(s => {
          // Defensive: never assign a non-array (downstream reads `.length`/maps).
          s.allGroups = Array.isArray(response.groups) ? response.groups : []
          s.groupsLoading = false
          s.groupsError = null
          s.groupsInitialized = true
        })
      } catch (error) {
        console.error('Failed to load user groups:', error)
        set(s => {
          s.groupsLoading = false
          s.groupsError = error instanceof Error ? error.message : 'Failed to load groups'
        })
        throw error
      }
    }
    return {
      loadAllGroups,
      // Load groups for a specific provider; uses cached user groups.
      loadGroupsForProvider: async (providerId: string, force = false): Promise<void> => {
        const state = get()
        const existing = state.providerGroups.get(providerId)
        if (existing?.loading && !force) return
        // Fresh (< 30s) and not forcing → cached.
        if (
          !force &&
          existing?.lastFetched &&
          Date.now() - existing.lastFetched < 30000 &&
          !existing.error
        ) {
          return
        }
        set(s => {
          s.providerGroups.set(providerId, {
            providerId,
            groups: existing?.groups || [],
            loading: true,
            error: null,
            lastFetched: existing?.lastFetched || null,
          })
        })
        try {
          const groups = await ApiClient.LlmProvider.getGroups({ provider_id: providerId })
          set(s => {
            s.providerGroups.set(providerId, {
              providerId,
              groups,
              loading: false,
              error: null,
              lastFetched: Date.now(),
            })
          })
        } catch (error) {
          console.error(`Failed to load groups for provider ${providerId}:`, error)
          set(s => {
            s.providerGroups.set(providerId, {
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
      clearProviderGroups: (providerId: string): void => {
        set(s => {
          s.providerGroups.delete(providerId)
        })
      },
      clearAllProviderGroups: (): void => {
        set(s => {
          s.providerGroups.clear()
        })
      },
      getProviderGroupsData: (providerId: string): ProviderGroups | undefined =>
        get().providerGroups.get(providerId),
    }
  },
  init: ({ on, get, set, actions }) => {
    // Invalidate the groups cache + reload on any group change.
    const handleGroupChange = () => {
      set(s => {
        s.groupsInitialized = false
      })
      void actions.loadAllGroups()
    }
    on('group.created', handleGroupChange)
    on('group.updated', handleGroupChange)
    on('group.deleted', handleGroupChange)
    // When groups are assigned to a provider, update the cache directly.
    on('llm_provider.groups_changed', async event => {
      const { providerId, groupIds } = event.data
      await actions.loadAllGroups()
      const assignedGroups = get().allGroups.filter(g => groupIds.includes(g.id))
      set(s => {
        s.providerGroups.set(providerId, {
          providerId,
          groups: assignedGroups,
          loading: false,
          error: null,
          lastFetched: Date.now(),
        })
      })
    })
    // `GET /api/groups` requires groups::read (not user-held). Guard the eager
    // load so a viewer without it (reaching the provider page via
    // llm_providers::read) doesn't 403 at store-mount.
    if (hasPermissionNow(Permissions.GroupsRead)) {
      void actions.loadAllGroups()
    }
  },
})

export const useProviderGroupCardStore = ProviderGroupCard.store
