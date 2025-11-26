import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import type { User } from '@/api-client/types'
import { Stores } from '@/core/stores'

interface UserGroupsDrawerState {
  isOpen: boolean
  user: User | null

  openUserGroupsDrawer: (user: User) => void
  closeUserGroupsDrawer: () => void

  __init__: {
    __store__: () => void
  }
  __destroy__?: () => void
}

export const useUserGroupsDrawerStore = create<UserGroupsDrawerState>()(
  subscribeWithSelector(
    immer(
      (set, get): UserGroupsDrawerState => ({
        isOpen: false,
        user: null,

        openUserGroupsDrawer: (user: User) => {
          set({ isOpen: true, user })
        },

        closeUserGroupsDrawer: () => {
          set({ isOpen: false, user: null })
        },

        __init__: {
          __store__: () => {
            const GROUP = 'UserGroupsDrawerStore'
            const eventBus = Stores.EventBus

            // Close drawer when user is deleted
            eventBus.on(
              'user.deleted',
              async event => {
                const { userId } = event.data
                const state = get()
                if (state.user?.id === userId) {
                  get().closeUserGroupsDrawer()
                }
              },
              GROUP,
            )

            // Could refresh if currently viewing that group (optional)
            eventBus.on(
              'group.deleted',
              async () => {
                // Optional: could trigger a refresh of the groups list
              },
              GROUP,
            )
          },
        },

        __destroy__: () => {
          Stores.EventBus.removeGroupListeners('UserGroupsDrawerStore')
        },
      }),
    ),
  ),
)
