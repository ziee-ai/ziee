import type { User } from '@/api-client/types'
import { defineStore } from '@/core/store-kit'

export const EditUserDrawer = defineStore('EditUserDrawer', {
  state: { isOpen: false, editingUser: null as User | null },
  actions: set => ({
    openEditUserDrawer: (user: User) => set({ isOpen: true, editingUser: user }),
    closeEditUserDrawer: () => set({ isOpen: false, editingUser: null }),
  }),
  init: ({ on, get, set, actions }) => {
    on('user.updated', event => {
      if (get().editingUser?.id === event.data.user.id) set({ editingUser: event.data.user })
    })
    on('user.deleted', event => {
      if (get().editingUser?.id === event.data.userId) actions.closeEditUserDrawer()
    })
  },
})

export const useEditUserDrawerStore = EditUserDrawer.store
