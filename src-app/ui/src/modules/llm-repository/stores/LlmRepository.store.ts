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
} from '@/modules/llm-repository/events'
import { Stores } from '@/core/stores'

interface LlmRepositoryState {
  // Data
  repositories: LlmRepository[]
  isInitialized: boolean

  // Pagination state — drives the settings page's <Pagination>.
  // Backend `LlmRepository.list` returns
  // `{ repositories, total, page, per_page }`.
  currentPage: number
  pageSize: number
  total: number

  // Loading states
  loading: boolean
  creating: boolean
  updating: boolean
  deleting: boolean
  testing: boolean

  // Error state
  error: string | null

  // Actions
  loadLlmRepositories: (page?: number, pageSize?: number) => Promise<void>
  createLlmRepository: (
    data: CreateLlmRepositoryRequest,
  ) => Promise<LlmRepository>
  updateLlmRepository: (
    id: string,
    data: UpdateLlmRepositoryRequest,
  ) => Promise<LlmRepository>
  deleteLlmRepository: (id: string) => Promise<void>
  testLlmRepositoryConnection: (
    data: TestRepositoryConnectionRequest,
  ) => Promise<{ success: boolean; message: string }>
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
      // Pagination defaults match the settings page's
      // pageSizeOptions={['5','10','20','50']}.
      currentPage: 1,
      pageSize: 10,
      total: 0,
      loading: false,
      creating: false,
      updating: false,
      deleting: false,
      testing: false,
      error: null,

      // Repository actions. Page-changes from the UI re-invoke this
      // with explicit page/pageSize; `__init__` calls it with no args
      // for the initial load. We only skip a fresh call when one is
      // already in flight — explicit pagination requests always run.
      loadLlmRepositories: async (page?: number, pageSize?: number) => {
        const state = get()
        if (state.loading) {
          return
        }
        const nextPage = page ?? state.currentPage
        const nextPageSize = pageSize ?? state.pageSize
        try {
          set({ loading: true, error: null })

          const response = await ApiClient.LlmRepository.list({
            page: nextPage,
            per_page: nextPageSize,
          })

          set({
            repositories: response.repositories,
            total: response.total,
            currentPage: response.page,
            pageSize: response.per_page,
            isInitialized: true,
            loading: false,
          })
        } catch (error) {
          set({
            error:
              error instanceof Error
                ? error.message
                : 'Failed to load repositories',
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
            console.error(
              'Failed to emit llm repository created event:',
              eventError,
            )
          }

          set({ creating: false })

          return repository
        } catch (error) {
          set({
            error:
              error instanceof Error
                ? error.message
                : 'Failed to create repository',
            creating: false,
          })
          throw error
        }
      },

      updateLlmRepository: async (
        id: string,
        data: UpdateLlmRepositoryRequest,
      ) => {
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
            console.error(
              'Failed to emit llm repository updated event:',
              eventError,
            )
          }

          set({ updating: false })

          return repository
        } catch (error) {
          set({
            error:
              error instanceof Error
                ? error.message
                : 'Failed to update repository',
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
            console.error(
              'Failed to emit llm repository deleted event:',
              eventError,
            )
          }

          set({ deleting: false })
        } catch (error) {
          set({
            error:
              error instanceof Error
                ? error.message
                : 'Failed to delete repository',
            deleting: false,
          })
          throw error
        }
      },

      testLlmRepositoryConnection: async (
        data: TestRepositoryConnectionRequest,
      ) => {
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
        // 09-llm-repository F-02 closure made api_key / password /
        // token write-only — they're never returned by the server.
        // We can't introspect them from the response anymore. The
        // server-side validate_auth_config_for_create/update already
        // refuses to persist an api_key auth_type with an empty
        // api_key, so if a repo exists with auth_type != 'none' we
        // can trust the credentials are set.
        //
        // The basic_auth case is partially checkable client-side
        // because username (non-secret) still rides along — but
        // password is hidden, so we treat the same way for
        // consistency.
        if (repository.auth_type === 'none') {
          return true
        }
        // auth_type is api_key / basic_auth / bearer_token — server
        // guarantees credentials are populated.
        return true
      },

      __init__: {
        __store__: () => {
          const eventBus = Stores.EventBus
          const GROUP = 'LlmRepositoryStore'

          // Subscribe to llm_repository.created
          eventBus.on(
            'llm_repository.created',
            async event => {
              const { repository } = event.data
              set(state => ({
                repositories: [...state.repositories, repository],
              }))
            },
            GROUP,
          )

          // Subscribe to llm_repository.updated
          eventBus.on(
            'llm_repository.updated',
            async event => {
              const { repository } = event.data
              set(state => ({
                repositories: state.repositories.map(r =>
                  r.id === repository.id ? repository : r,
                ),
              }))
            },
            GROUP,
          )

          // Subscribe to llm_repository.deleted
          eventBus.on(
            'llm_repository.deleted',
            async event => {
              const { repositoryId } = event.data
              set(state => ({
                repositories: state.repositories.filter(
                  r => r.id !== repositoryId,
                ),
              }))
            },
            GROUP,
          )
        },
        repositories: () => get().loadLlmRepositories(),
      },

      __destroy__: () => {
        Stores.EventBus.removeGroupListeners('LlmRepositoryStore')
      },
    }),
  ),
)
