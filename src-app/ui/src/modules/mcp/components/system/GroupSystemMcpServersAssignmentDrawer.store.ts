import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import type { Group } from '@/api-client/types'
import { Stores } from '@/core/stores'

interface GroupSystemMcpServersAssignmentState {
  isOpen: boolean
  selectedGroup: Group | null

  openDrawer: (group: Group) => void
  closeDrawer: () => void

  __init__: {
    __store__: () => void
  }
  __destroy__?: () => void
}

export const useGroupSystemMcpServersAssignmentStore =
  create<GroupSystemMcpServersAssignmentState>()(
    subscribeWithSelector(
      immer((set, get) => ({
        isOpen: false,
        selectedGroup: null,

        __init__: {
          __store__: () => {
            const GROUP = 'GroupSystemMcpServersAssignmentDrawerStore'
            const eventBus = Stores.EventBus

            // Subscribe to group.updated
            eventBus.on(
              'group.updated',
              async event => {
                const { group } = event.data
                const state = get()

                if (state.selectedGroup?.id === group.id) {
                  set(state => {
                    state.selectedGroup = group
                  })
                }
              },
              GROUP,
            )

            // Subscribe to group.deleted
            eventBus.on(
              'group.deleted',
              async event => {
                const { groupId } = event.data
                const state = get()

                if (state.selectedGroup?.id === groupId) {
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
            'GroupSystemMcpServersAssignmentDrawerStore',
          )
        },
      })),
    ),
  )
