import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import type { Group } from '@/api-client/types'
import { Stores } from '@/core/stores'

interface GroupSystemWorkflowsAssignmentState {
  isOpen: boolean
  selectedGroup: Group | null

  openDrawer: (group: Group) => void
  closeDrawer: () => void

  __init__: {
    __store__: () => void
  }
  __destroy__?: () => void
}

/**
 * Open/selected-group state for the System Workflows assignment drawer.
 * Mirrors the MCP `GroupSystemMcpServersAssignmentDrawer.store`.
 */
export const useGroupSystemWorkflowsAssignmentStore =
  create<GroupSystemWorkflowsAssignmentState>()(
    subscribeWithSelector(
      immer((set, get) => ({
        isOpen: false,
        selectedGroup: null,

        __init__: {
          __store__: () => {
            const GROUP = 'GroupSystemWorkflowsAssignmentDrawerStore'
            const eventBus = Stores.EventBus

            eventBus.on(
              'group.updated',
              async event => {
                const { group } = event.data
                if (get().selectedGroup?.id === group.id) {
                  set(state => {
                    state.selectedGroup = group
                  })
                }
              },
              GROUP,
            )

            eventBus.on(
              'group.deleted',
              async event => {
                const { groupId } = event.data
                if (get().selectedGroup?.id === groupId) {
                  get().closeDrawer()
                }
              },
              GROUP,
            )
          },
        },

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

        __destroy__: () => {
          Stores.EventBus.removeGroupListeners(
            'GroupSystemWorkflowsAssignmentDrawerStore',
          )
        },
      })),
    ),
  )
