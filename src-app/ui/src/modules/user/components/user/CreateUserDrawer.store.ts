import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'

interface CreateUserDrawerState {
  isOpen: boolean

  openCreateUserDrawer: () => void
  closeCreateUserDrawer: () => void

  __init__: {
    __store__: () => void
  }
}

export const useCreateUserDrawerStore = create<CreateUserDrawerState>()(
  subscribeWithSelector(
    immer(
      (set): CreateUserDrawerState => ({
        isOpen: false,

        openCreateUserDrawer: () => {
          set({ isOpen: true })
        },

        closeCreateUserDrawer: () => {
          set({ isOpen: false })
        },

        __init__: {
          __store__: () => {
            // No event subscriptions needed for create-only drawer
          },
        },
      }),
    ),
  ),
)
