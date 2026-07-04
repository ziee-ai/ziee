import { defineStore } from '@/core/store-kit'

export const CreateUserDrawer = defineStore('CreateUserDrawer', {
  state: { isOpen: false },
  actions: set => ({
    openCreateUserDrawer: () => set({ isOpen: true }),
    closeCreateUserDrawer: () => set({ isOpen: false }),
  }),
})

export const useCreateUserDrawerStore = CreateUserDrawer.store
