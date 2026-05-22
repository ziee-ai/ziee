import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import type {
  HubAssistant,
  Assistant,
  CreateAssistantFromHubRequest,
} from '@/api-client/types'
import { Stores } from '@/core/stores'

interface HubAssistantsState {
  assistants: HubAssistant[]
  version: string | null
  loading: boolean
  creating: boolean
  error: string | null

  // Actions
  loadAssistants: () => Promise<void>
  refreshFromGitHub: () => Promise<void>
  createFromHub: (request: CreateAssistantFromHubRequest) => Promise<Assistant>

  // Lazy initialization
  __init__: {
    assistants: () => Promise<void>
    __store__?: () => void
  }
}

export const useHubAssistantsStore = create<HubAssistantsState>()(
  subscribeWithSelector(
    immer(
      (set, get): HubAssistantsState => ({
        assistants: [],
        version: null,
        loading: false,
        creating: false,
        error: null,

        loadAssistants: async () => {
          const state = get()
          if (state.loading) return

          set({ loading: true, error: null })
          try {
            // Load with user's locale
            const locale = 'en' // TODO: Get from user settings
            const assistants = await ApiClient.Hub.getAssistants({
              lang: locale,
            })
            const versionInfo = await ApiClient.Hub.getAssistantsVersion()

            set({
              assistants,
              version: versionInfo.version,
              loading: false,
            })
          } catch (error: any) {
            set({
              error: error.message || 'Failed to load hub assistants',
              loading: false,
            })
          }
        },

        refreshFromGitHub: async () => {
          set({ loading: true, error: null })
          try {
            // Call category-specific refresh endpoint
            const result = await ApiClient.Hub.refreshAssistants()

            // Reload if updated
            if (result.updated) {
              await get().loadAssistants()
            }

            set({ loading: false })
          } catch (error: any) {
            set({
              error: error.message || 'Failed to refresh hub assistants',
              loading: false,
            })
            throw error
          }
        },

        createFromHub: async (
          request: CreateAssistantFromHubRequest,
        ): Promise<Assistant> => {
          set({ creating: true, error: null })
          try {
            const response = await ApiClient.Hub.createAssistantFromHub(request)

            // Update the hub assistant's created_ids directly from response
            set(state => {
              const assistant = state.assistants.find(
                a => a.id === request.hub_id,
              )
              if (assistant) {
                if (!assistant.created_ids) {
                  assistant.created_ids = []
                }
                assistant.created_ids.push(response.hub_tracking.entity_id)
              }
              state.creating = false
            })

            return response.assistant
          } catch (error: any) {
            set({
              error: error.message || 'Failed to create assistant from hub',
              creating: false,
            })
            throw error
          }
        },

        __init__: {
          __store__: () => {
            Stores.EventBus.on(
              'assistant.deleted',
              event => {
                const { assistantId } = event.data
                set(state => {
                  for (const assistant of state.assistants) {
                    if (assistant.created_ids) {
                      assistant.created_ids = assistant.created_ids.filter(
                        id => id !== assistantId,
                      )
                    }
                  }
                })
              },
              'HubAssistantsStore',
            )
          },
          assistants: () => get().loadAssistants(),
        },
      }),
    ),
  ),
)
