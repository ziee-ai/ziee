import type { User } from '@/api-client/types'
import { defineStore } from '@ziee/framework/store-kit'

export const ResetPasswordDrawer = defineStore('ResetPasswordDrawer', {
  state: { isOpen: false, user: null as User | null },
  actions: set => ({
    openResetPasswordDrawer: (user: User) => set({ isOpen: true, user }),
    closeResetPasswordDrawer: () => set({ isOpen: false, user: null }),
  }),
  init: ({ on, get, actions }) => {
    on('user.deleted', event => {
      if (get().user?.id === event.data.userId) actions.closeResetPasswordDrawer()
    })
  },
})

export const useResetPasswordDrawerStore = ResetPasswordDrawer.store
