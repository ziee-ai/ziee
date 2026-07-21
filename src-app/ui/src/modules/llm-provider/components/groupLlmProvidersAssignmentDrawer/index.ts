import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { groupLlmProvidersAssignmentState, type GroupLlmProvidersAssignmentDrawerState } from './state'
import type { Actions } from './actions.gen'

const GroupLlmProvidersAssignmentDef = defineStore<GroupLlmProvidersAssignmentDrawerState, Actions>('GroupLlmProvidersAssignment', {
  immer: true,
  state: groupLlmProvidersAssignmentState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, get, set, actions }) => {
    on('group.updated', event => {
      if (get().selectedGroup?.id === event.data.group.id) set({ selectedGroup: event.data.group })
    })
    on('group.deleted', event => {
      if (get().selectedGroup?.id === event.data.groupId) actions.closeDrawer()
    })
  },
})
export const GroupLlmProvidersAssignment = registerLazyStore(GroupLlmProvidersAssignmentDef)
export const useGroupLlmProvidersAssignmentStore = GroupLlmProvidersAssignmentDef.store
