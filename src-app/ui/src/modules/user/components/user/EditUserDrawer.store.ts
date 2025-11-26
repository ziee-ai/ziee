import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import type { User } from '@/api-client/types'
import { Stores } from '@/core/stores'

interface EditUserDrawerState {
  isOpen: boolean
  editingUser: User | null

  openEditUserDrawer: (user: User) => void
  closeEditUserDrawer: () => void

  __init__: {
    __store__: () => void
  }
  __destroy__?: () => void
}

export const useEditUserDrawerStore = create<EditUserDrawerState>()(
  subscribeWithSelector(
    immer(
      (set, get): EditUserDrawerState => ({
        isOpen: false,
        editingUser: null,

        openEditUserDrawer: (user: User) => {
          set({ isOpen: true, editingUser: user })
        },

        closeEditUserDrawer: () => {
          set({ isOpen: false, editingUser: null })
        },

        __init__: {
          __store__: () => {
            const GROUP = 'EditUserDrawerStore'
            const eventBus = Stores.EventBus

            // Update editingUser when user is updated
            eventBus.on(
              'user.updated',
              async event => {
                const { user } = event.data
                const state = get()
                if (state.editingUser?.id === user.id) {
                  set({ editingUser: user })
                }
              },
              GROUP,
            )

            // Close drawer when user is deleted
            eventBus.on(
              'user.deleted',
              async event => {
                const { userId } = event.data
                const state = get()
                if (state.editingUser?.id === userId) {
                  get().closeEditUserDrawer()
                }
              },
              GROUP,
            )
          },
        },

        __destroy__: () => {
          Stores.EventBus.removeGroupListeners('EditUserDrawerStore')
        },
      }),
    ),
  ),
)
