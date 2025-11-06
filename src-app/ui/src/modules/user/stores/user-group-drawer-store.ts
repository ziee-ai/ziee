import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import type { Group } from '@/api-client/types'

interface UserGroupDrawerState {
  // State
  isOpen: boolean
  editingGroup: Group | null

  // Actions
  openUserGroupDrawer: (group: Group) => void
  closeUserGroupDrawer: () => void
}

export const useUserGroupDrawerStore = create<UserGroupDrawerState>()(
  subscribeWithSelector(
    (set): UserGroupDrawerState => ({
      isOpen: false,
      editingGroup: null,

      openUserGroupDrawer: (group: Group) => {
        set({ isOpen: true, editingGroup: group })
      },

      closeUserGroupDrawer: () => {
        set({ isOpen: false, editingGroup: null })
      },
    }),
  ),
)
