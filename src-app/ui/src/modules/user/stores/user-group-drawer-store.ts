import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import type { Group } from '@/api-client/types'
import { Stores } from '@/core/stores'

interface UserGroupDrawerState {
  // State
  isOpen: boolean
  editingGroup: Group | null

  // Actions
  openUserGroupDrawer: (group: Group) => void
  closeUserGroupDrawer: () => void

  // Initialization
  __init__: {
    __store__: () => void
  }
}

export const useUserGroupDrawerStore = create<UserGroupDrawerState>()(
  subscribeWithSelector(
    (set, get): UserGroupDrawerState => ({
      isOpen: false,
      editingGroup: null,

      __init__: {
        __store__: () => {
          const eventBus = Stores.EventBus

          // Subscribe to group.updated
          eventBus.on('group.updated', async event => {
            const { group } = event.data
            const state = get()

            if (state.editingGroup?.id === group.id) {
              set({ editingGroup: group })
            }
          })

          // Subscribe to group.deleted
          eventBus.on('group.deleted', async event => {
            const { groupId } = event.data
            const state = get()

            if (state.editingGroup?.id === groupId) {
              get().closeUserGroupDrawer()
            }
          })
        },
      },

      openUserGroupDrawer: (group: Group) => {
        set({ isOpen: true, editingGroup: group })
      },

      closeUserGroupDrawer: () => {
        set({ isOpen: false, editingGroup: null })
      },
    }),
  ),
)
