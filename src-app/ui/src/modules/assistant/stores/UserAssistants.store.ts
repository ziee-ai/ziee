import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import {
  Permissions,
  type Assistant,
  type CreateAssistantRequest,
  type UpdateAssistantRequest,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { Stores } from '@/core/stores'
import {
  emitAssistantCreated,
  emitAssistantDeleted,
  emitAssistantUpdated,
} from '@/modules/assistant/events'

interface UserAssistantsState {
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
    __store__?: () => void
    assistants: () => Promise<void>
  }

  __destroy__?: () => void

  // Actions
  loadUserAssistants: (page?: number, pageSize?: number) => Promise<void>
  createUserAssistant: (data: CreateAssistantRequest) => Promise<Assistant>
  updateUserAssistant: (
    id: string,
    data: UpdateAssistantRequest,
  ) => Promise<Assistant>
  deleteUserAssistant: (id: string) => Promise<void>
  clearUserAssistantsStoreError: () => void
  getUserDefaultAssistant: () => Assistant | undefined
}

export const useUserAssistantsStore = create<UserAssistantsState>()(
  subscribeWithSelector(
    immer(
      (set, get): UserAssistantsState => ({
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
          __store__: () => {
            const eventBus = Stores.EventBus
            const GROUP = 'UserAssistantsStore'

            // Reload the current page on any local mutation so pagination
            // (total / current page) stays consistent.
            const reloadCurrent = () => void get().loadUserAssistants()

            eventBus.on('assistant.created', reloadCurrent, GROUP)
            eventBus.on('assistant.updated', reloadCurrent, GROUP)
            eventBus.on('assistant.deleted', reloadCurrent, GROUP)

            // Remote sync: refetch on a remote change or on (re)connect.
            // Self-gate the sync-driven refetch: `sync:reconnect` fires for
            // every store regardless of the user's permissions, so without
            // this an assistants-read-less user would 403 on reconnect.
            const reload = () => {
              if (!hasPermissionNow(Permissions.AssistantsRead)) return
              void get().loadUserAssistants()
            }
            eventBus.on('sync:assistant', reload, GROUP)
            eventBus.on('sync:reconnect', reload, GROUP)
          },
          assistants: () => get().loadUserAssistants(),
        },

        // Actions
        loadUserAssistants: async (
          page?: number,
          pageSize?: number,
        ): Promise<void> => {
          if (!hasPermissionNow(Permissions.AssistantsRead)) {
            return
          }
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
              error:
                error instanceof Error
                  ? error.message
                  : 'Failed to load assistants',
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

            try {
              await emitAssistantCreated(assistant)
            } catch (eventError) {
              console.error(
                'Failed to emit assistant created event:',
                eventError,
              )
            }

            // Reload to maintain pagination consistency.
            await get().loadUserAssistants()

            set({ creating: false })

            return assistant
          } catch (error) {
            set({
              error:
                error instanceof Error
                  ? error.message
                  : 'Failed to create assistant',
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

            try {
              await emitAssistantUpdated(assistant)
            } catch (eventError) {
              console.error(
                'Failed to emit assistant updated event:',
                eventError,
              )
            }

            await get().loadUserAssistants()

            set({ updating: false })

            return assistant
          } catch (error) {
            set({
              error:
                error instanceof Error
                  ? error.message
                  : 'Failed to update assistant',
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
              console.error(
                'Failed to emit assistant deleted event:',
                eventError,
              )
            }

            await get().loadUserAssistants()

            set({ deleting: false })
          } catch (error) {
            set({
              error:
                error instanceof Error
                  ? error.message
                  : 'Failed to delete assistant',
              deleting: false,
            })
            throw error
          }
        },

        clearUserAssistantsStoreError: (): void => {
          set({ error: null })
        },

        getUserDefaultAssistant: (): Assistant | undefined => {
          return get().assistants.find(a => a.is_default)
        },

        __destroy__: () => {
          Stores.EventBus.removeGroupListeners('UserAssistantsStore')
        },
      }),
    ),
  ),
)
