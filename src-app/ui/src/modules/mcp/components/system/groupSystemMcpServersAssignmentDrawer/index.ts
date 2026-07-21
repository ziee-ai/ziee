import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { groupSystemMcpServersAssignmentDrawerState, type GroupSystemMcpServersAssignmentDrawerState } from './state'
import type { Actions } from './actions.gen'

const GroupSystemMcpServersAssignmentDrawerDef = defineStore<GroupSystemMcpServersAssignmentDrawerState, Actions>('GroupSystemMcpServersAssignment', {
  immer: true,
  state: groupSystemMcpServersAssignmentDrawerState,
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
export const GroupSystemMcpServersAssignment = registerLazyStore(GroupSystemMcpServersAssignmentDrawerDef)
export const useGroupSystemMcpServersAssignmentStore = GroupSystemMcpServersAssignmentDrawerDef.store
