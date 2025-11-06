import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { enableMapSet } from 'immer'
import { ApiClient } from '@/api-client'
import type { Assistant, CreateAssistantRequest, UpdateAssistantRequest } from '@/api-client/types'

// Enable Map and Set support in Immer
enableMapSet()

interface UserAssistantsState {
  // Data
  assistants: Map<string, Assistant>
  isInitialized: boolean

  // Loading states
  loading: boolean
  creating: boolean
  updating: boolean
  deleting: boolean

  // Error state
  error: string | null

  __init__: {
    assistants: () => Promise<void>
  }

  // Actions
  loadUserAssistants: () => Promise<void>
  createUserAssistant: (data: CreateAssistantRequest) => Promise<Assistant>
  updateUserAssistant: (id: string, data: UpdateAssistantRequest) => Promise<Assistant>
  deleteUserAssistant: (id: string) => Promise<void>
  clearUserAssistantsStoreError: () => void
  getUserDefaultAssistant: () => Assistant | undefined
}

export const useUserAssistantsStore = create<UserAssistantsState>()(
  subscribeWithSelector(
    immer(
      (set, get): UserAssistantsState => ({
        // Initial state
        assistants: new Map<string, Assistant>(),
        isInitialized: false,
        loading: false,
        creating: false,
        updating: false,
        deleting: false,
        error: null,
        __init__: {
          assistants: () => get().loadUserAssistants(),
        },

        // Actions
        loadUserAssistants: async (): Promise<void> => {
          const state = get()
          if (state.isInitialized || state.loading) {
            return
          }
          try {
            set({ loading: true, error: null })

            const response = await ApiClient.Assistant.list({
              page: 1,
              limit: 50,
            })

            set({
              assistants: new Map(
                response.assistants.map((assistant: Assistant) => [assistant.id, assistant]),
              ),
              isInitialized: true,
              loading: false,
            })
          } catch (error) {
            set({
              error:
                error instanceof Error ? error.message : 'Failed to load assistants',
              loading: false,
            })
            throw error
          }
        },

        createUserAssistant: async (
          data: CreateAssistantRequest,
        ): Promise<Assistant> => {
          try {
            set({ creating: true, error: null })

            const assistant = await ApiClient.Assistant.create(data)

            set(state => {
              if (data.is_default) {
                // Set all other assistants' is_default to false
                state.assistants.forEach((a: Assistant) => {
                  a.is_default = false
                })
              }
              // Add the new assistant
              state.assistants.set(assistant.id, assistant)
              state.creating = false
            })

            return assistant
          } catch (error) {
            set({
              error:
                error instanceof Error ? error.message : 'Failed to create assistant',
              creating: false,
            })
            throw error
          }
        },

        updateUserAssistant: async (
          id: string,
          data: UpdateAssistantRequest,
        ): Promise<Assistant> => {
          try {
            set({ updating: true, error: null })

            const assistant = await ApiClient.Assistant.update({
              id,
              ...data,
            })

            set(state => {
              if (data.is_default) {
                // Set all other assistants' is_default to false
                state.assistants.forEach((a: Assistant, assistantId: string) => {
                  if (assistantId !== id) {
                    a.is_default = false
                  }
                })
              }
              // Update the assistant
              state.assistants.set(id, assistant)
              state.updating = false
            })

            return assistant
          } catch (error) {
            set({
              error:
                error instanceof Error ? error.message : 'Failed to update assistant',
              updating: false,
            })
            throw error
          }
        },

        deleteUserAssistant: async (id: string): Promise<void> => {
          try {
            set({ deleting: true, error: null })

            await ApiClient.Assistant.delete({ id })

            set(state => {
              state.assistants.delete(id)
              state.deleting = false
            })
          } catch (error) {
            set({
              error:
                error instanceof Error ? error.message : 'Failed to delete assistant',
              deleting: false,
            })
            throw error
          }
        },

        clearUserAssistantsStoreError: (): void => {
          set({ error: null })
        },

        getUserDefaultAssistant: (): Assistant | undefined => {
          return Array.from(get().assistants.values())
            .find(a => a.is_default)
        },
      }),
    ),
  ),
)
