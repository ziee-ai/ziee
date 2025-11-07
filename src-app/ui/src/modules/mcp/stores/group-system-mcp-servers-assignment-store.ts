import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import type { Group } from '@/api-client/types'

interface GroupSystemMcpServersAssignmentState {
  isOpen: boolean
  selectedGroup: Group | null
  lastUpdated: number | null

  openDrawer: (group: Group) => void
  closeDrawer: () => void
  markUpdated: () => void
}

export const useGroupSystemMcpServersAssignmentStore = create<GroupSystemMcpServersAssignmentState>()(
  subscribeWithSelector(
    immer(set => ({
      isOpen: false,
      selectedGroup: null,
      lastUpdated: null,

      openDrawer: (group: Group) => {
        set(state => {
          state.isOpen = true
          state.selectedGroup = group
        })
      },

      closeDrawer: () => {
        set(state => {
          state.isOpen = false
          state.selectedGroup = null
        })
      },

      markUpdated: () => {
        set(state => {
          state.lastUpdated = Date.now()
        })
      },
    })),
  ),
)
