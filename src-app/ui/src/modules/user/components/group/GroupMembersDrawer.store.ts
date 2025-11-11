import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import type { Group } from '@/api-client/types'
import { Stores } from '@/core/stores'

interface GroupMembersDrawerState {
  // State
  isOpen: boolean
  selectedGroup: Group | null

  // Actions
  openGroupMembersDrawer: (group: Group) => void
  closeGroupMembersDrawer: () => void

  // Initialization
  __init__: {
    __store__: () => void
  }
  __destroy__?: () => void
}

export const useGroupMembersDrawerStore = create<GroupMembersDrawerState>()(
  subscribeWithSelector(
    (set, get): GroupMembersDrawerState => ({
      isOpen: false,
      selectedGroup: null,

      __init__: {
        __store__: () => {
          const GROUP = 'GroupMembersDrawerStore'
          const eventBus = Stores.EventBus

          // Subscribe to group.updated
          eventBus.on('group.updated', async event => {
            const { group } = event.data
            const state = get()

            if (state.selectedGroup?.id === group.id) {
              set({ selectedGroup: group })
            }
          }, GROUP)

          // Subscribe to group.deleted
          eventBus.on('group.deleted', async event => {
            const { groupId } = event.data
            const state = get()

            if (state.selectedGroup?.id === groupId) {
              get().closeGroupMembersDrawer()
            }
          }, GROUP)
        },
      },

      openGroupMembersDrawer: (group: Group) => {
        set({ isOpen: true, selectedGroup: group })
      },

      closeGroupMembersDrawer: () => {
        set({ isOpen: false, selectedGroup: null })
      },

      __destroy__: () => {
        Stores.EventBus.removeGroupListeners('GroupMembersDrawerStore')
      },
    }),
  ),
)
