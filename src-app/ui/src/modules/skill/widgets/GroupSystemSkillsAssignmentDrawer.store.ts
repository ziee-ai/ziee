import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import type { Group } from '@/api-client/types'
import { Stores } from '@/core/stores'

interface GroupSystemSkillsAssignmentState {
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
 * Open/selected-group state for the System Skills assignment drawer. Mirrors
 * the MCP `GroupSystemMcpServersAssignmentDrawer.store`: closes on delete of
 * the selected group, tracks rename via group.updated.
 */
export const useGroupSystemSkillsAssignmentStore =
  create<GroupSystemSkillsAssignmentState>()(
    subscribeWithSelector(
      immer((set, get) => ({
        isOpen: false,
        selectedGroup: null,

        __init__: {
          __store__: () => {
            const GROUP = 'GroupSystemSkillsAssignmentDrawerStore'
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
            'GroupSystemSkillsAssignmentDrawerStore',
          )
        },
      })),
    ),
  )
