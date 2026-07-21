import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { groupSystemWorkflowsAssignmentState, type GroupSystemWorkflowsAssignmentDrawerState } from './state'
import type { Actions } from './actions.gen'

const GroupSystemWorkflowsAssignmentDef = defineStore<GroupSystemWorkflowsAssignmentDrawerState, Actions>(
  'GroupSystemWorkflowsAssignment',
  {
    immer: true,
    state: groupSystemWorkflowsAssignmentState,
    actions: import.meta.glob('./actions/*.ts'),
    init: ({ on, get, set, actions }) => {
      on('group.updated', event => {
        if (get().selectedGroup?.id === event.data.group.id) set({ selectedGroup: event.data.group })
      })
      on('group.deleted', event => {
        if (get().selectedGroup?.id === event.data.groupId) actions.closeDrawer()
      })
    },
  },
)
export const GroupSystemWorkflowsAssignment = registerLazyStore(GroupSystemWorkflowsAssignmentDef)
export const useGroupSystemWorkflowsAssignmentStore = GroupSystemWorkflowsAssignmentDef.store
