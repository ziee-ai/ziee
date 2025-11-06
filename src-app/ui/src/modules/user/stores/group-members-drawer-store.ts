import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import type { Group } from '@/api-client/types'

interface GroupMembersDrawerState {
  // State
  isOpen: boolean
  selectedGroup: Group | null

  // Actions
  openGroupMembersDrawer: (group: Group) => void
  closeGroupMembersDrawer: () => void
}

export const useGroupMembersDrawerStore = create<GroupMembersDrawerState>()(
  subscribeWithSelector(
    (set): GroupMembersDrawerState => ({
      isOpen: false,
      selectedGroup: null,

      openGroupMembersDrawer: (group: Group) => {
        set({ isOpen: true, selectedGroup: group })
      },

      closeGroupMembersDrawer: () => {
        set({ isOpen: false, selectedGroup: null })
      },
    }),
  ),
)
