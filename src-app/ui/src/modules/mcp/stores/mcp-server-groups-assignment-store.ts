import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'

interface McpServerGroupsAssignmentState {
  isOpen: boolean
  selectedServerId: string | null

  openDrawer: (serverId: string) => void
  closeDrawer: () => void
}

export const useMcpServerGroupsAssignmentStore = create<McpServerGroupsAssignmentState>()(
  subscribeWithSelector(
    immer(set => ({
      isOpen: false,
      selectedServerId: null,

      openDrawer: (serverId: string) => {
        set(state => {
          state.isOpen = true
          state.selectedServerId = serverId
        })
      },

      closeDrawer: () => {
        set(state => {
          state.isOpen = false
          state.selectedServerId = null
        })
      },
    })),
  ),
)
