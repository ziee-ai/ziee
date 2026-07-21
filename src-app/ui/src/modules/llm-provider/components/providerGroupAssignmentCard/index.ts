import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import { providerGroupAssignmentCardState, type ProviderGroupAssignmentCardState } from './state'
import type { Actions } from './actions.gen'

const ProviderGroupAssignmentCardDef = defineStore<ProviderGroupAssignmentCardState, Actions>(
  'ProviderGroupAssignmentCard',
  {
    immer: true,
    state: providerGroupAssignmentCardState,
    actions: import.meta.glob('./actions/*.ts'),
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
  },
)

export const ProviderGroupAssignmentCard = registerLazyStore(ProviderGroupAssignmentCardDef)
export const useProviderGroupAssignmentCardStore = ProviderGroupAssignmentCardDef.store
