import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { ApiClient } from '@/api-client'
import type {
  LlmRepository,
  CreateLlmRepositoryRequest,
  UpdateLlmRepositoryRequest,
  TestRepositoryConnectionRequest,
} from '@/api-client/types'
import {
  emitLlmRepositoryCreated,
  emitLlmRepositoryUpdated,
  emitLlmRepositoryDeleted,
} from '../events'
import { Stores } from '@/core/stores'

interface LlmRepositoryState {
  // Data
  repositories: LlmRepository[]
  isInitialized: boolean

  // Loading states
  loading: boolean
  creating: boolean
  updating: boolean
  deleting: boolean
  testing: boolean

  // Error state
  error: string | null

  // Actions
  loadLlmRepositories: () => Promise<void>
  createLlmRepository: (data: CreateLlmRepositoryRequest) => Promise<LlmRepository>
  updateLlmRepository: (id: string, data: UpdateLlmRepositoryRequest) => Promise<LlmRepository>
  deleteLlmRepository: (id: string) => Promise<void>
  testLlmRepositoryConnection: (data: TestRepositoryConnectionRequest) => Promise<{ success: boolean; message: string }>
  clearLlmRepositoryStoreError: () => void
  findLlmRepositoryById: (id: string) => LlmRepository | undefined
  llmRepositoryHasCredentials: (repository: LlmRepository) => boolean

  __init__: {
    __store__?: () => void
    repositories: () => Promise<void>
  }

  __destroy__?: () => void
}

export const useLlmRepositoryStore = create<LlmRepositoryState>()(
  subscribeWithSelector(
    (set, get): LlmRepositoryState => ({
      // Initial state
      repositories: [],
      isInitialized: false,
      loading: false,
      creating: false,
      updating: false,
      deleting: false,
      testing: false,
      error: null,

      // Repository actions
      loadLlmRepositories: async () => {
        const state = get()
        if (state.isInitialized || state.loading) {
          return
        }
        try {
          set({ loading: true, error: null })

          const response = await ApiClient.LlmRepository.list({
            page: 1,
            per_page: 50,
          })

          set({
            repositories: response.repositories,
            isInitialized: true,
            loading: false,
          })
        } catch (error) {
          set({
            error:
              error instanceof Error ? error.message : 'Failed to load repositories',
            loading: false,
          })
          throw error
        }
      },

      createLlmRepository: async (data: CreateLlmRepositoryRequest) => {
        const state = get()
        if (state.creating) {
          return Promise.resolve(null as any)
        }

        try {
          set({ creating: true, error: null })

          const repository = await ApiClient.LlmRepository.create(data)

          // Emit event after successful API call
          // Event handler will update state (no manual state update here)
          try {
            await emitLlmRepositoryCreated(repository)
          } catch (eventError) {
            console.error('Failed to emit llm repository created event:', eventError)
          }

          set({ creating: false })

          return repository
        } catch (error) {
          set({
            error:
              error instanceof Error ? error.message : 'Failed to create repository',
            creating: false,
          })
          throw error
        }
      },

      updateLlmRepository: async (id: string, data: UpdateLlmRepositoryRequest) => {
        const state = get()
        if (state.updating) {
          return Promise.resolve(null as any)
        }

        try {
          set({ updating: true, error: null })

          const repository = await ApiClient.LlmRepository.update({
            repository_id: id,
            ...data,
          })

          // Emit event after successful API call
          // Event handler will update state (no manual state update here)
          try {
            await emitLlmRepositoryUpdated(repository)
          } catch (eventError) {
            console.error('Failed to emit llm repository updated event:', eventError)
          }

          set({ updating: false })

          return repository
        } catch (error) {
          set({
            error:
              error instanceof Error ? error.message : 'Failed to update repository',
            updating: false,
          })
          throw error
        }
      },

      deleteLlmRepository: async (id: string) => {
        const state = get()
        if (state.deleting) {
          return
        }

        try {
          set({ deleting: true, error: null })

          await ApiClient.LlmRepository.delete({ repository_id: id })

          // Emit event after successful API call
          // Event handler will update state (no manual state update here)
          try {
            await emitLlmRepositoryDeleted(id)
          } catch (eventError) {
            console.error('Failed to emit llm repository deleted event:', eventError)
          }

          set({ deleting: false })
        } catch (error) {
          set({
            error:
              error instanceof Error ? error.message : 'Failed to delete repository',
            deleting: false,
          })
          throw error
        }
      },

      testLlmRepositoryConnection: async (data: TestRepositoryConnectionRequest) => {
        const state = get()
        if (state.testing) {
          return {
            success: false,
            message: 'Repository connection test already in progress',
          }
        }

        try {
          set({ testing: true, error: null })

          const result = await ApiClient.LlmRepository.test(data)

          set({ testing: false })

          return result
        } catch (error) {
          set({
            error:
              error instanceof Error
                ? error.message
                : 'Failed to test repository connection',
            testing: false,
          })
          throw error
        }
      },

      clearLlmRepositoryStoreError: () => {
        set({ error: null })
      },

      findLlmRepositoryById: (id: string) => {
        return get().repositories.find(r => r.id === id)
      },

      llmRepositoryHasCredentials: (repository: LlmRepository) => {
        // If auth type is none, no credentials are needed
        if (repository.auth_type === 'none') {
          return true
        }

        // Check if auth_config exists
        if (!repository.auth_config) {
          return false
        }

        // Check credentials based on auth type
        switch (repository.auth_type) {
          case 'api_key':
            return !!(
              repository.auth_config.api_key && repository.auth_config.api_key.trim()
            )

          case 'basic_auth':
            return !!(
              repository.auth_config.username &&
              repository.auth_config.username.trim() &&
              repository.auth_config.password &&
              repository.auth_config.password.trim()
            )

          case 'bearer_token':
            return !!(
              repository.auth_config.token && repository.auth_config.token.trim()
            )

          default:
            return false
        }
      },

      __init__: {
        __store__: () => {
          const eventBus = Stores.EventBus
          const GROUP = 'LlmRepositoryStore'

          // Subscribe to llm_repository.created
          eventBus.on('llm_repository.created', async event => {
            const { repository } = event.data
            set(state => ({
              repositories: [...state.repositories, repository],
            }))
          }, GROUP)

          // Subscribe to llm_repository.updated
          eventBus.on('llm_repository.updated', async event => {
            const { repository } = event.data
            set(state => ({
              repositories: state.repositories.map(r =>
                r.id === repository.id ? repository : r,
              ),
            }))
          }, GROUP)

          // Subscribe to llm_repository.deleted
          eventBus.on('llm_repository.deleted', async event => {
            const { repositoryId } = event.data
            set(state => ({
              repositories: state.repositories.filter(r => r.id !== repositoryId),
            }))
          }, GROUP)
        },
        repositories: () => get().loadLlmRepositories(),
      },

      __destroy__: () => {
        Stores.EventBus.removeGroupListeners('LlmRepositoryStore')
      },
    }),
  ),
)
