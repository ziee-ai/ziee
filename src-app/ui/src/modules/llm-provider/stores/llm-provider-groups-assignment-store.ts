import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'

/**
 * Store for managing the LLM Provider Groups Assignment Drawer state.
 * This drawer allows assigning/removing user groups to/from an LLM provider.
 */
interface LlmProviderGroupsAssignmentState {
  isOpen: boolean
  selectedProviderId: string | null
  openDrawer: (providerId: string) => void
  closeDrawer: () => void
}

export const useLlmProviderGroupsAssignmentStore =
  create<LlmProviderGroupsAssignmentState>()(
    subscribeWithSelector(
      (set): LlmProviderGroupsAssignmentState => ({
        isOpen: false,
        selectedProviderId: null,

        openDrawer: (providerId: string) => {
          set({ isOpen: true, selectedProviderId: providerId })
        },

        closeDrawer: () => {
          set({ isOpen: false, selectedProviderId: null })
        },
      }),
    ),
  )
