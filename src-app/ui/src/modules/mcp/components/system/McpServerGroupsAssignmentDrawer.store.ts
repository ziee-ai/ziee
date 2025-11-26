import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { Stores } from '@/core/stores'

interface McpServerGroupsAssignmentState {
  isOpen: boolean
  selectedServerId: string | null

  openDrawer: (serverId: string) => void
  closeDrawer: () => void

  __init__: {
    __store__: () => void
  }
  __destroy__?: () => void
}

export const useMcpServerGroupsAssignmentStore =
  create<McpServerGroupsAssignmentState>()(
    subscribeWithSelector(
      immer((set, get) => ({
        isOpen: false,
        selectedServerId: null,

        __init__: {
          __store__: () => {
            const GROUP = 'McpServerGroupsAssignmentDrawerStore'
            const eventBus = Stores.EventBus

            // Subscribe to mcp_server.deleted
            eventBus.on(
              'mcp_server.deleted',
              async event => {
                const { serverId } = event.data
                const state = get()

                if (state.selectedServerId === serverId) {
                  get().closeDrawer()
                }
              },
              GROUP,
            )
          },
        },

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

        __destroy__: () => {
          Stores.EventBus.removeGroupListeners(
            'McpServerGroupsAssignmentDrawerStore',
          )
        },
      })),
    ),
  )
