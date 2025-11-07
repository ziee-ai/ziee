import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import type { User } from '@/api-client/types'
import { Stores } from '@/core/stores'

interface ResetPasswordDrawerState {
  isOpen: boolean
  user: User | null

  openResetPasswordDrawer: (user: User) => void
  closeResetPasswordDrawer: () => void

  __init__: {
    __store__: () => void
  }
}

export const useResetPasswordDrawerStore = create<ResetPasswordDrawerState>()(
  subscribeWithSelector(
    immer(
      (set, get): ResetPasswordDrawerState => ({
        isOpen: false,
        user: null,

        openResetPasswordDrawer: (user: User) => {
          set({ isOpen: true, user })
        },

        closeResetPasswordDrawer: () => {
          set({ isOpen: false, user: null })
        },

        __init__: {
          __store__: () => {
            const eventBus = Stores.EventBus

            // Close drawer when user is deleted
            eventBus.on('user.deleted', async event => {
              const { userId } = event.data
              const state = get()
              if (state.user?.id === userId) {
                get().closeResetPasswordDrawer()
              }
            })
          },
        },
      }),
    ),
  ),
)
