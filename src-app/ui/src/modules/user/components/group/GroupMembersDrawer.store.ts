import type { Group } from '@/api-client/types'
import { defineStore } from '@/core/store-kit'

export const GroupMembersDrawer = defineStore('GroupMembersDrawer', {
  state: { isOpen: false, selectedGroup: null as Group | null },
  actions: set => ({
    openGroupMembersDrawer: (group: Group) => set({ isOpen: true, selectedGroup: group }),
    closeGroupMembersDrawer: () => set({ isOpen: false, selectedGroup: null }),
  }),
  init: ({ on, get, set, actions }) => {
    on('group.updated', event => {
      if (get().selectedGroup?.id === event.data.group.id) set({ selectedGroup: event.data.group })
    })
    on('group.deleted', event => {
      if (get().selectedGroup?.id === event.data.groupId) actions.closeGroupMembersDrawer()
    })
  },
})

export const useGroupMembersDrawerStore = GroupMembersDrawer.store
