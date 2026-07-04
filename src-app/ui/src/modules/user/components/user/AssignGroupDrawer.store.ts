import type { User } from '@/api-client/types'
import { defineStore } from '@/core/store-kit'

export const AssignGroupDrawer = defineStore('AssignGroupDrawer', {
  state: { isOpen: false, user: null as User | null },
  actions: set => ({
    openAssignGroupDrawer: (user: User) => set({ isOpen: true, user }),
    closeAssignGroupDrawer: () => set({ isOpen: false, user: null }),
  }),
  init: ({ on, get, actions }) => {
    on('user.deleted', event => {
      if (get().user?.id === event.data.userId) actions.closeAssignGroupDrawer()
    })
  },
})

export const useAssignGroupDrawerStore = AssignGroupDrawer.store
