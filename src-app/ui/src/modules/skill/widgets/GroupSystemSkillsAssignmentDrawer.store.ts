import type { Group } from '@/api-client/types'
import { defineStore } from '@/core/store-kit'

/** Open/selected-group state for the System Skills assignment drawer. */
export const GroupSystemSkillsAssignment = defineStore('GroupSystemSkillsAssignment', {
  state: { isOpen: false, selectedGroup: null as Group | null },
  actions: set => ({
    openDrawer: (group: Group) => set({ isOpen: true, selectedGroup: group }),
    closeDrawer: () => set({ isOpen: false, selectedGroup: null }),
  }),
  init: ({ on, get, set, actions }) => {
    on('group.updated', event => {
      if (get().selectedGroup?.id === event.data.group.id) set({ selectedGroup: event.data.group })
    })
    on('group.deleted', event => {
      if (get().selectedGroup?.id === event.data.groupId) actions.closeDrawer()
    })
  },
})

export const useGroupSystemSkillsAssignmentStore = GroupSystemSkillsAssignment.store
