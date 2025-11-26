import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import type { Group } from '@/api-client/types'
import { Stores } from '@/core/stores'

/**
 * Store for managing the Group LLM Providers Assignment Drawer state.
 * This drawer allows assigning/removing LLM Providers to/from a user group.
 */
interface GroupLlmProvidersAssignmentState {
  isOpen: boolean
  selectedGroup: Group | null
  openDrawer: (group: Group) => void
  closeDrawer: () => void

  __init__: {
    __store__: () => void
  }
  __destroy__?: () => void
}

export const useGroupLlmProvidersAssignmentStore =
  create<GroupLlmProvidersAssignmentState>()(
    subscribeWithSelector(
      (set, get): GroupLlmProvidersAssignmentState => ({
        isOpen: false,
        selectedGroup: null,

        __init__: {
          __store__: () => {
            const GROUP = 'GroupLlmProvidersAssignmentDrawerStore'
            const eventBus = Stores.EventBus

            // Subscribe to group.updated
            eventBus.on(
              'group.updated',
              async event => {
                const { group } = event.data
                const state = get()

                if (state.selectedGroup?.id === group.id) {
                  set({ selectedGroup: group })
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
          set({ isOpen: true, selectedGroup: group })
        },

        closeDrawer: () => {
          set({ isOpen: false, selectedGroup: null })
        },

        __destroy__: () => {
          Stores.EventBus.removeGroupListeners(
            'GroupLlmProvidersAssignmentDrawerStore',
          )
        },
      }),
    ),
  )
