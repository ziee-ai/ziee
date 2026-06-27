import { enableMapSet } from 'immer'
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
import { Stores } from '@/core/stores'
import { hasPermissionNow } from '@/core/permissions'
import {
  emitAssistantCreated,
  emitAssistantDeleted,
  emitAssistantUpdated,
} from '@/modules/assistant/events'

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
    __store__?: () => void
    assistants: () => Promise<void>
  }

  __destroy__?: () => void

  // Actions
  loadUserAssistants: (force?: boolean) => Promise<void>
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
        assistants: new Map<string, Assistant>(),
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

            // Subscribe to assistant.created
            eventBus.on(
              'assistant.created',
              async event => {
                const { assistant } = event.data
                set(state => {
                  state.assistants.set(assistant.id, assistant)
                })
              },
              GROUP,
            )

            // Subscribe to assistant.updated
            eventBus.on(
              'assistant.updated',
              async event => {
                const { assistant } = event.data
                set(state => {
                  state.assistants.set(assistant.id, assistant)
                })
              },
              GROUP,
            )

            // Subscribe to assistant.deleted
            eventBus.on(
              'assistant.deleted',
              async event => {
                const { assistantId } = event.data
                set(state => {
                  state.assistants.delete(assistantId)
                })
              },
              GROUP,
            )

            // Remote sync: refetch on a remote change or on (re)connect.
            // Self-gate the sync-driven refetch: `sync:reconnect` fires for
            // every store regardless of the user's permissions, so without
            // this an assistants-read-less user would 403 on reconnect. Auth
            // is already populated by the time these events fire (unlike the
            // mount-time __init__ load), so the snapshot check is reliable here.
            const reload = () => {
              if (!hasPermissionNow(Permissions.AssistantsRead)) return
              void get().loadUserAssistants(true)
            }
            eventBus.on('sync:assistant', reload, GROUP)
            eventBus.on('sync:reconnect', reload, GROUP)
          },
          assistants: () => get().loadUserAssistants(),
        },

        // Actions
        loadUserAssistants: async (force = false): Promise<void> => {
          const state = get()
          if ((state.isInitialized && !force) || state.loading) {
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
                response.assistants.map((assistant: Assistant) => [
                  assistant.id,
                  assistant,
                ]),
              ),
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

            // Emit event after successful API call
            // Event handler will update state (no manual state update here)
            try {
              await emitAssistantCreated(assistant)
            } catch (eventError) {
              console.error(
                'Failed to emit assistant created event:',
                eventError,
              )
            }

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

            // Emit event after successful API call
            // Event handler will update state (no manual state update here)
            try {
              await emitAssistantUpdated(assistant)
            } catch (eventError) {
              console.error(
                'Failed to emit assistant updated event:',
                eventError,
              )
            }

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

            // Emit event after successful API call
            // Event handler will update state (no manual state update here)
            try {
              await emitAssistantDeleted(id)
            } catch (eventError) {
              console.error(
                'Failed to emit assistant deleted event:',
                eventError,
              )
            }

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
          return Array.from(get().assistants.values()).find(a => a.is_default)
        },

        __destroy__: () => {
          Stores.EventBus.removeGroupListeners('UserAssistantsStore')
        },
      }),
    ),
  ),
)
