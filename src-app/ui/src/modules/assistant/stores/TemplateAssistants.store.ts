import { ApiClient } from '@/api-client'
import { type Assistant, type CreateAssistantRequest, type UpdateAssistantRequest } from '@/api-client/types'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'
import {
  emitAssistantTemplateCreated,
  emitAssistantTemplateDeleted,
  emitAssistantTemplateUpdated,
} from '@/modules/assistant/events'

export const TemplateAssistants = defineStore('TemplateAssistants', {
  immer: true,
  state: {
    assistants: [] as Assistant[],
    total: 0,
    currentPage: 1,
    pageSize: 10,
    isInitialized: false,
    loading: false,
    creating: false,
    updating: false,
    deleting: false,
    error: null as string | null,
  },
  actions: (set, get) => {
    const loadTemplateAssistants = async (page?: number, pageSize?: number): Promise<void> => {
      if (!hasPermissionNow(Permissions.AssistantsTemplateRead)) return
      try {
        const currentState = get()
        const requestPage = page || currentState.currentPage
        const requestPageSize = pageSize || currentState.pageSize
        // Skip if already initialized and loading first page without explicit page.
        if (currentState.isInitialized && currentState.loading && !page) return
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
            error instanceof Error ? error.message : 'Failed to load template assistants',
          loading: false,
        })
        throw error
      }
    }
    return {
      loadTemplateAssistants,
      createTemplateAssistant: async (
        data: CreateAssistantRequest,
      ): Promise<Assistant | undefined> => {
        if (get().creating) return
        try {
          set({ creating: true, error: null })
          const assistant = await ApiClient.AssistantTemplate.create(data)
          try {
            await emitAssistantTemplateCreated(assistant)
          } catch (eventError) {
            console.error('Failed to emit assistant template created event:', eventError)
          }
          await loadTemplateAssistants()
          set({ creating: false })
          return assistant
        } catch (error) {
          set({
            error:
              error instanceof Error ? error.message : 'Failed to create template assistant',
            creating: false,
          })
          throw error
        }
      },
      updateTemplateAssistant: async (
        id: string,
        data: UpdateAssistantRequest,
      ): Promise<Assistant | undefined> => {
        if (get().updating) return
        try {
          set({ updating: true, error: null })
          const assistant = await ApiClient.AssistantTemplate.update({ id, ...data })
          try {
            await emitAssistantTemplateUpdated(assistant)
          } catch (eventError) {
            console.error('Failed to emit assistant template updated event:', eventError)
          }
          await loadTemplateAssistants()
          set({ updating: false })
          return assistant
        } catch (error) {
          set({
            error:
              error instanceof Error ? error.message : 'Failed to update template assistant',
            updating: false,
          })
          throw error
        }
      },
      deleteTemplateAssistant: async (id: string): Promise<void> => {
        if (get().deleting) return
        try {
          set({ deleting: true, error: null })
          await ApiClient.AssistantTemplate.delete({ id })
          try {
            await emitAssistantTemplateDeleted(id)
          } catch (eventError) {
            console.error('Failed to emit assistant template deleted event:', eventError)
          }
          await loadTemplateAssistants()
          set({ deleting: false })
        } catch (error) {
          set({
            error:
              error instanceof Error ? error.message : 'Failed to delete template assistant',
            deleting: false,
          })
          throw error
        }
      },
      clearTemplateAssistantsStoreError: (): void => {
        set({ error: null })
      },
      getTemplateDefaultAssistant: (): Assistant | undefined =>
        get().assistants.find(a => a.is_default),
    }
  },
  init: ({ on, actions }) => {
    const reload = () => void actions.loadTemplateAssistants()
    on('assistant_template.created', reload)
    on('assistant_template.updated', reload)
    on('assistant_template.deleted', reload)
    // The load action self-gates on AssistantsTemplateRead.
    on('sync:assistant_template', reload)
    on('sync:reconnect', reload)
    void actions.loadTemplateAssistants()
  },
})

export const useTemplateAssistantsStore = TemplateAssistants.store
