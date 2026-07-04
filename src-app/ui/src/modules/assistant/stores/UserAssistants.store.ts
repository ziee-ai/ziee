import { ApiClient } from '@/api-client'
import {
  type Assistant,
  type CreateAssistantRequest,
  Permissions,
  type UpdateAssistantRequest,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@/core/store-kit'
import {
  emitAssistantCreated,
  emitAssistantDeleted,
  emitAssistantUpdated,
} from '@/modules/assistant/events'

export const UserAssistants = defineStore('UserAssistants', {
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
    const loadUserAssistants = async (page?: number, pageSize?: number): Promise<void> => {
      if (!hasPermissionNow(Permissions.AssistantsRead)) return
      try {
        const currentState = get()
        const requestPage = page || currentState.currentPage
        const requestPageSize = pageSize || currentState.pageSize
        set({ loading: true, error: null })
        const response = await ApiClient.Assistant.list({
          page: requestPage,
          limit: requestPageSize,
        })
        set({
          assistants: response?.assistants ?? [],
          total: response?.total ?? 0,
          currentPage: requestPage,
          pageSize: requestPageSize,
          isInitialized: true,
          loading: false,
        })
      } catch (error) {
        set({
          error: error instanceof Error ? error.message : 'Failed to load assistants',
          loading: false,
        })
        throw error
      }
    }
    return {
      loadUserAssistants,
      createUserAssistant: async (data: CreateAssistantRequest): Promise<Assistant> => {
        try {
          set({ creating: true, error: null })
          const assistant = await ApiClient.Assistant.create(data)
          try {
            await emitAssistantCreated(assistant)
          } catch (eventError) {
            console.error('Failed to emit assistant created event:', eventError)
          }
          // Reload to maintain pagination consistency.
          await loadUserAssistants()
          set({ creating: false })
          return assistant
        } catch (error) {
          set({
            error: error instanceof Error ? error.message : 'Failed to create assistant',
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
          const assistant = await ApiClient.Assistant.update({ id, ...data })
          try {
            await emitAssistantUpdated(assistant)
          } catch (eventError) {
            console.error('Failed to emit assistant updated event:', eventError)
          }
          await loadUserAssistants()
          set({ updating: false })
          return assistant
        } catch (error) {
          set({
            error: error instanceof Error ? error.message : 'Failed to update assistant',
            updating: false,
          })
          throw error
        }
      },
      deleteUserAssistant: async (id: string): Promise<void> => {
        try {
          set({ deleting: true, error: null })
          await ApiClient.Assistant.delete({ id })
          try {
            await emitAssistantDeleted(id)
          } catch (eventError) {
            console.error('Failed to emit assistant deleted event:', eventError)
          }
          await loadUserAssistants()
          set({ deleting: false })
        } catch (error) {
          set({
            error: error instanceof Error ? error.message : 'Failed to delete assistant',
            deleting: false,
          })
          throw error
        }
      },
      clearUserAssistantsStoreError: (): void => {
        set({ error: null })
      },
      getUserDefaultAssistant: (): Assistant | undefined =>
        get().assistants.find(a => a.is_default),
    }
  },
  init: ({ on, actions }) => {
    // Reload the current page on any local mutation so pagination stays consistent.
    const reloadCurrent = () => void actions.loadUserAssistants()
    on('assistant.created', reloadCurrent)
    on('assistant.updated', reloadCurrent)
    on('assistant.deleted', reloadCurrent)
    // Remote sync: self-gate (reconnect fires for every store regardless of perms).
    const reload = () => {
      if (!hasPermissionNow(Permissions.AssistantsRead)) return
      void actions.loadUserAssistants()
    }
    on('sync:assistant', reload)
    on('sync:reconnect', reload)
    void actions.loadUserAssistants()
  },
})

export const useUserAssistantsStore = UserAssistants.store
