import type { User } from '@/api-client/types'
import { defineStore } from '@ziee/framework/store-kit'

export const UserGroupsDrawer = defineStore('UserGroupsDrawer', {
  state: { isOpen: false, user: null as User | null },
  actions: set => ({
    openUserGroupsDrawer: (user: User) => set({ isOpen: true, user }),
    closeUserGroupsDrawer: () => set({ isOpen: false, user: null }),
  }),
  init: ({ on, get, actions }) => {
    on('user.deleted', event => {
      if (get().user?.id === event.data.userId) actions.closeUserGroupsDrawer()
    })
  },
})

export const useUserGroupsDrawerStore = UserGroupsDrawer.store
