import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { Stores } from '@/core/stores'

/**
 * Store for managing the LLM Provider Groups Assignment Drawer state.
 * This drawer allows assigning/removing user groups to/from an LLM provider.
 */
interface LlmProviderGroupsAssignmentState {
  isOpen: boolean
  selectedProviderId: string | null
  openDrawer: (providerId: string) => void
  closeDrawer: () => void

  __init__: {
    __store__: () => void
  }
}

export const useLlmProviderGroupsAssignmentStore =
  create<LlmProviderGroupsAssignmentState>()(
    subscribeWithSelector(
      (set, get): LlmProviderGroupsAssignmentState => ({
        isOpen: false,
        selectedProviderId: null,

        __init__: {
          __store__: () => {
            const eventBus = Stores.EventBus

            // Subscribe to llm_provider.deleted
            eventBus.on('llm_provider.deleted', async event => {
              const { providerId } = event.data
              const state = get()

              if (state.selectedProviderId === providerId) {
                get().closeDrawer()
              }
            })
          },
        },

        openDrawer: (providerId: string) => {
          set({ isOpen: true, selectedProviderId: providerId })
        },

        closeDrawer: () => {
          set({ isOpen: false, selectedProviderId: null })
        },
      }),
    ),
  )
