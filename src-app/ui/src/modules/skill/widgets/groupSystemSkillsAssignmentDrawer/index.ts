import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { groupSystemSkillsAssignmentDrawerState, type GroupSystemSkillsAssignmentDrawerState } from './state'
import type { Actions } from './actions.gen'

const GroupSystemSkillsAssignmentDef = defineStore<GroupSystemSkillsAssignmentDrawerState, Actions>('GroupSystemSkillsAssignment', {
  immer: true,
  state: groupSystemSkillsAssignmentDrawerState,
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
export const GroupSystemSkillsAssignment = registerLazyStore(GroupSystemSkillsAssignmentDef)
export const useGroupSystemSkillsAssignmentStore = GroupSystemSkillsAssignmentDef.store
