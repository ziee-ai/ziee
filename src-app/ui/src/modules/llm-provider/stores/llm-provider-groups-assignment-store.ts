import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'

/**
 * Store for managing the LLM Provider Groups Assignment Drawer state.
 * This drawer allows assigning/removing user groups to/from an LLM provider.
 */
interface LlmProviderGroupsAssignmentState {
  isOpen: boolean
  selectedProviderId: string | null
  lastUpdated: number | null
  openDrawer: (providerId: string) => void
  closeDrawer: () => void
  markUpdated: () => void
}

export const useLlmProviderGroupsAssignmentStore =
  create<LlmProviderGroupsAssignmentState>()(
    subscribeWithSelector(
      (set): LlmProviderGroupsAssignmentState => ({
        isOpen: false,
        selectedProviderId: null,
        lastUpdated: null,

        openDrawer: (providerId: string) => {
          set({ isOpen: true, selectedProviderId: providerId })
        },

        closeDrawer: () => {
          set({ isOpen: false, selectedProviderId: null })
        },

        markUpdated: () => {
          set({ lastUpdated: Date.now() })
        },
      }),
    ),
  )
