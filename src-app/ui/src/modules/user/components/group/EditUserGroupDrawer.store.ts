import type { Group } from '@/api-client/types'
import { defineStore } from '@/core/store-kit'

export const EditUserGroupDrawer = defineStore('EditUserGroupDrawer', {
  state: { isOpen: false, editingGroup: null as Group | null },
  actions: set => ({
    openUserGroupDrawer: (group: Group) => set({ isOpen: true, editingGroup: group }),
    closeUserGroupDrawer: () => set({ isOpen: false, editingGroup: null }),
  }),
  init: ({ on, get, set, actions }) => {
    on('group.updated', event => {
      if (get().editingGroup?.id === event.data.group.id) set({ editingGroup: event.data.group })
    })
    on('group.deleted', event => {
      if (get().editingGroup?.id === event.data.groupId) actions.closeUserGroupDrawer()
    })
  },
})

export const useUserGroupDrawerStore = EditUserGroupDrawer.store
