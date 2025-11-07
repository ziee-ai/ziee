import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import type { Group } from '@/api-client/types'

/**
 * Store for managing the Group LLM Providers Assignment Drawer state.
 * This drawer allows assigning/removing LLM Providers to/from a user group.
 */
interface GroupLlmProvidersAssignmentState {
  isOpen: boolean
  selectedGroup: Group | null
  openDrawer: (group: Group) => void
  closeDrawer: () => void
}

export const useGroupLlmProvidersAssignmentStore =
  create<GroupLlmProvidersAssignmentState>()(
    subscribeWithSelector(
      (set): GroupLlmProvidersAssignmentState => ({
        isOpen: false,
        selectedGroup: null,

        openDrawer: (group: Group) => {
          set({ isOpen: true, selectedGroup: group })
        },

        closeDrawer: () => {
          set({ isOpen: false, selectedGroup: null })
        },
      }),
    ),
  )
