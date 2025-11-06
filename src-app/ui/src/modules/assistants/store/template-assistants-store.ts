import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import type { Assistant, CreateAssistantRequest, UpdateAssistantRequest } from '@/api-client/types'

interface TemplateAssistantsState {
  // Data
  assistants: Assistant[]
  total: number
  currentPage: number
  pageSize: number
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
  loadTemplateAssistants: (page?: number, pageSize?: number) => Promise<void>
  createTemplateAssistant: (data: CreateAssistantRequest) => Promise<Assistant | undefined>
  updateTemplateAssistant: (id: string, data: UpdateAssistantRequest) => Promise<Assistant | undefined>
  deleteTemplateAssistant: (id: string) => Promise<void>
  clearTemplateAssistantsStoreError: () => void
  getTemplateDefaultAssistant: () => Assistant | undefined
}

export const useTemplateAssistantsStore = create<TemplateAssistantsState>()(
  subscribeWithSelector(
    immer(
      (set, get): TemplateAssistantsState => ({
        // Initial state
        assistants: [],
        total: 0,
        currentPage: 1,
        pageSize: 10,
        isInitialized: false,
        loading: false,
        creating: false,
        updating: false,
        deleting: false,
        error: null,
        __init__: {
          assistants: () => get().loadTemplateAssistants(),
        },

        // Actions
        loadTemplateAssistants: async (page?: number, pageSize?: number): Promise<void> => {
          try {
            const currentState = get()
            const requestPage = page || currentState.currentPage
            const requestPageSize = pageSize || currentState.pageSize

            // Skip if already initialized and loading first page without explicit page parameter
            if (currentState.isInitialized && currentState.loading && !page) {
              return
            }

            set({ loading: true, error: null })

            const response = await ApiClient.AssistantTemplate.list({
              page: requestPage,
              limit: requestPageSize,
            })

            set({
              assistants: response.assistants,
              total: response.total,
              currentPage: requestPage,
              pageSize: requestPageSize,
              isInitialized: true,
              loading: false,
            })
          } catch (error) {
            set({
              error:
                error instanceof Error
                  ? error.message
                  : 'Failed to load template assistants',
              loading: false,
            })
            throw error
          }
        },

        createTemplateAssistant: async (
          data: CreateAssistantRequest,
        ): Promise<Assistant | undefined> => {
          const state = get()
          if (state.creating) {
            return
          }

          try {
            set({ creating: true, error: null })

            const assistant = await ApiClient.AssistantTemplate.create(data)

            // Reload the list to maintain pagination consistency
            await get().loadTemplateAssistants()

            set({ creating: false })

            return assistant
          } catch (error) {
            set({
              error:
                error instanceof Error
                  ? error.message
                  : 'Failed to create template assistant',
              creating: false,
            })
            throw error
          }
        },

        updateTemplateAssistant: async (
          id: string,
          data: UpdateAssistantRequest,
        ): Promise<Assistant | undefined> => {
          const state = get()
          if (state.updating) {
            return
          }

          try {
            set({ updating: true, error: null })

            const assistant = await ApiClient.AssistantTemplate.update({
              id,
              ...data,
            })

            // Reload the list to maintain pagination consistency
            await get().loadTemplateAssistants()

            set({ updating: false })

            return assistant
          } catch (error) {
            set({
              error:
                error instanceof Error
                  ? error.message
                  : 'Failed to update template assistant',
              updating: false,
            })
            throw error
          }
        },

        deleteTemplateAssistant: async (id: string): Promise<void> => {
          const state = get()
          if (state.deleting) {
            return
          }

          try {
            set({ deleting: true, error: null })

            await ApiClient.AssistantTemplate.delete({ id })

            // Reload the list to maintain pagination consistency
            await get().loadTemplateAssistants()

            set({ deleting: false })
          } catch (error) {
            set({
              error:
                error instanceof Error
                  ? error.message
                  : 'Failed to delete template assistant',
              deleting: false,
            })
            throw error
          }
        },

        clearTemplateAssistantsStoreError: (): void => {
          set({ error: null })
        },

        getTemplateDefaultAssistant: (): Assistant | undefined => {
          return get().assistants.find(a => a.is_default)
        },
      }),
    ),
  ),
)
