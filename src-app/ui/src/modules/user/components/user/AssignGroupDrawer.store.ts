import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import type { User } from '@/api-client/types'
import { Stores } from '@/core/stores'

interface AssignGroupDrawerState {
  isOpen: boolean
  user: User | null

  openAssignGroupDrawer: (user: User) => void
  closeAssignGroupDrawer: () => void

  __init__: {
    __store__: () => void
  }
  __destroy__?: () => void
}

export const useAssignGroupDrawerStore = create<AssignGroupDrawerState>()(
  subscribeWithSelector(
    immer(
      (set, get): AssignGroupDrawerState => ({
        isOpen: false,
        user: null,

        openAssignGroupDrawer: (user: User) => {
          set({ isOpen: true, user })
        },

        closeAssignGroupDrawer: () => {
          set({ isOpen: false, user: null })
        },

        __init__: {
          __store__: () => {
            const GROUP = 'AssignGroupDrawerStore'
            const eventBus = Stores.EventBus

            // Close drawer when user is deleted
            eventBus.on(
              'user.deleted',
              async event => {
                const { userId } = event.data
                const state = get()
                if (state.user?.id === userId) {
                  get().closeAssignGroupDrawer()
                }
              },
              GROUP,
            )
          },
        },

        __destroy__: () => {
          Stores.EventBus.removeGroupListeners('AssignGroupDrawerStore')
        },
      }),
    ),
  ),
)
