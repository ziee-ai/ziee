import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'

/**
 * Store for managing the Provider Group Assignment Drawer state.
 * This drawer allows assigning/removing user groups to/from an LLM provider.
 */
interface ProviderGroupAssignmentState {
  isOpen: boolean
  selectedProviderId: string | null
  lastUpdated: number | null
  openDrawer: (providerId: string) => void
  closeDrawer: () => void
  markUpdated: () => void
}

export const useProviderGroupAssignmentStore =
  create<ProviderGroupAssignmentState>()(
    subscribeWithSelector(
      (set): ProviderGroupAssignmentState => ({
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
