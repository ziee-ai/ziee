import { ApiClient } from '@/api-client'
import {
  type CreateLlmRepositoryRequest,
  type LlmRepository,
  type LlmRepositoryWithHealthWarning,
  Permissions,
  type TestRepositoryConnectionRequest,
  type UpdateLlmRepositoryRequest,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@/core/store-kit'
import {
  emitLlmRepositoryAutoDisabled,
  emitLlmRepositoryCreated,
  emitLlmRepositoryDeleted,
  emitLlmRepositoryUpdated,
} from '@/modules/llm-repository/events'

export const LlmRepositoryStoreDef = defineStore('LlmRepository', {
  state: {
    repositories: [] as LlmRepository[],
    isInitialized: false,
    // Pagination defaults match the settings page's pageSizeOptions.
    currentPage: 1,
    pageSize: 10,
    total: 0,
    loading: false,
    creating: false,
    updating: false,
    deleting: false,
    testing: false,
    error: null as string | null,
  },
  actions: (set, get) => {
    // Page-changes re-invoke with explicit page/pageSize; init calls with no
    // args. Only skip when a load is already in flight.
    const loadLlmRepositories = async (page?: number, pageSize?: number) => {
      if (!hasPermissionNow(Permissions.LlmRepositoriesRead)) return
      const state = get()
      if (state.loading) return
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
          error: error instanceof Error ? error.message : 'Failed to load repositories',
          loading: false,
        })
        throw error
      }
    }
    return {
      loadLlmRepositories,
      createLlmRepository: async (
        data: CreateLlmRepositoryRequest,
      ): Promise<LlmRepositoryWithHealthWarning> => {
        if (get().creating) return Promise.resolve(null as any)
        try {
          set({ creating: true, error: null })
          // Response `{ repository, connection_warning? }` (flattened): the
          // backend probes when enabled:true and auto-flips on failure.
          const wrapped = await ApiClient.LlmRepository.create(data)
          try {
            await emitLlmRepositoryCreated(wrapped)
          } catch (eventError) {
            console.error('Failed to emit llm repository created event:', eventError)
          }
          // On downgrade, also emit auto_disabled so the settings page reloads
          // and renders the `unhealthy` Alert.
          if (wrapped.connection_warning) {
            try {
              await emitLlmRepositoryAutoDisabled(wrapped.id, wrapped.connection_warning.reason)
            } catch (eventError) {
              console.error('Failed to emit llm repository auto_disabled event:', eventError)
            }
          }
          set({ creating: false })
          return wrapped
        } catch (error) {
          set({
            error: error instanceof Error ? error.message : 'Failed to create repository',
            creating: false,
          })
          throw error
        }
      },
      updateLlmRepository: async (
        id: string,
        data: UpdateLlmRepositoryRequest,
      ): Promise<LlmRepository> => {
        if (get().updating) return Promise.resolve(null as any)
        try {
          set({ updating: true, error: null })
          const repository = await ApiClient.LlmRepository.update({ repository_id: id, ...data })
          try {
            await emitLlmRepositoryUpdated(repository)
          } catch (eventError) {
            console.error('Failed to emit llm repository updated event:', eventError)
          }
          set({ updating: false })
          return repository
        } catch (error) {
          // An enable-transition probe failure (400) leaves the row disabled +
          // `unhealthy` server-side. Emit auto_disabled so the list reloads
          // deterministically without waiting on the SSE round-trip.
          const code = (error as { error_code?: string })?.error_code
          if (code === 'LLM_REPOSITORY_ENABLE_FAILED_HEALTH_CHECK') {
            try {
              await emitLlmRepositoryAutoDisabled(
                id,
                error instanceof Error ? error.message : 'Connection probe failed',
              )
            } catch (eventError) {
              console.error('Failed to emit llm repository auto_disabled event:', eventError)
            }
          }
          set({
            error: error instanceof Error ? error.message : 'Failed to update repository',
            updating: false,
          })
          throw error
        }
      },
      deleteLlmRepository: async (id: string) => {
        if (get().deleting) return
        try {
          set({ deleting: true, error: null })
          await ApiClient.LlmRepository.delete({ repository_id: id })
          try {
            await emitLlmRepositoryDeleted(id)
          } catch (eventError) {
            console.error('Failed to emit llm repository deleted event:', eventError)
          }
          set({ deleting: false })
        } catch (error) {
          set({
            error: error instanceof Error ? error.message : 'Failed to delete repository',
            deleting: false,
          })
          throw error
        }
      },
      testLlmRepositoryConnection: async (
        data: TestRepositoryConnectionRequest,
      ): Promise<{ success: boolean; message: string }> => {
        if (get().testing) {
          return { success: false, message: 'Repository connection test already in progress' }
        }
        try {
          set({ testing: true, error: null })
          const result = await ApiClient.LlmRepository.test(data)
          set({ testing: false })
          return result
        } catch (error) {
          set({
            error:
              error instanceof Error ? error.message : 'Failed to test repository connection',
            testing: false,
          })
          throw error
        }
      },
      testLlmRepositoryById: async (
        id: string,
        overrides: UpdateLlmRepositoryRequest,
      ): Promise<{ success: boolean; message: string }> => {
        if (get().testing) {
          return { success: false, message: 'Repository connection test already in progress' }
        }
        try {
          set({ testing: true, error: null })
          // Endpoint takes the row id + UpdateLlmRepositoryRequest body; changed
          // fields override, empty secrets fall back server-side.
          const result = await ApiClient.LlmRepository.testById({ repository_id: id, ...overrides })
          set({ testing: false })
          // The test persisted a fresh health status; re-fetch + emit `updated`
          // so the list + open drawer reflect it (SSE round-trip is unreliable).
          try {
            const fresh = await ApiClient.LlmRepository.get({ repository_id: id })
            await emitLlmRepositoryUpdated(fresh)
          } catch (refreshError) {
            console.error('Failed to refresh repository after connection test:', refreshError)
          }
          return result
        } catch (error) {
          set({
            error:
              error instanceof Error ? error.message : 'Failed to test repository connection',
            testing: false,
          })
          throw error
        }
      },
      clearLlmRepositoryStoreError: () => {
        set({ error: null })
      },
      findLlmRepositoryById: (id: string): LlmRepository | undefined =>
        get().repositories.find(r => r.id === id),
      llmRepositoryHasCredentials: (repository: LlmRepository): boolean => {
        // Secrets (api_key/password/token) are write-only — never returned. The
        // server refuses to persist an api_key auth_type with an empty key, so a
        // row with auth_type != 'none' has credentials set.
        if (repository.auth_type === 'none') return true
        return true
      },
    }
  },
  init: ({ on, get, set, actions }) => {
    on('llm_repository.created', event => {
      set(state => ({ repositories: [...state.repositories, event.data.repository] }))
    })
    on('llm_repository.updated', event => {
      set(state => ({
        repositories: state.repositories.map(r =>
          r.id === event.data.repository.id ? event.data.repository : r,
        ),
      }))
    })
    on('llm_repository.deleted', event => {
      set(state => ({
        repositories: state.repositories.filter(r => r.id !== event.data.repositoryId),
      }))
    })
    // Cross-device sync: loadLlmRepositories self-gates + skips while in flight.
    const reload = () => void actions.loadLlmRepositories()
    on('sync:llm_repository', reload)
    on('sync:reconnect', reload)
    // Connection-health auto-disable: reload to surface the new health columns.
    // Boot-time auto-disables don't emit this (no EventBus yet at module init);
    // the settings page's mount-time load catches those.
    on('llm_repository.auto_disabled', () => {
      void actions.loadLlmRepositories(get().currentPage, get().pageSize)
    })
    void actions.loadLlmRepositories()
  },
})

export const useLlmRepositoryStore = LlmRepositoryStoreDef.store
