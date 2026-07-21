import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { llmRepositoryState, type LlmRepositoryState } from './state'
import type { Actions } from './actions.gen'
import { useAuthStore } from '@/modules/auth/Auth.store'
import { hasPermissionNow } from '@/core/permissions'
import { Permissions } from '@/api-client/permissions'

const LlmRepositoryDef = defineStore<LlmRepositoryState, Actions>('LlmRepository', {
  immer: true,
  state: llmRepositoryState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, get, set, actions, watch }) => {
    // Event-bus listeners: sync the in-memory list when the EventBus fires.
    on('llm_repository.created', event => {
      set(state => {
        state.repositories.push(event.data.repository)
      })
    })
    on('llm_repository.updated', event => {
      set(state => {
        const idx = state.repositories.findIndex(
          r => r.id === event.data.repository.id,
        )
        if (idx !== -1) state.repositories[idx] = event.data.repository
      })
    })
    on('llm_repository.deleted', event => {
      set(state => {
        state.repositories = state.repositories.filter(
          r => r.id !== event.data.repositoryId,
        )
      })
    })
    // Cross-device sync: reload when the server pushes a change.
    const reload = () => void actions.loadLlmRepositories()
    on('sync:llm_repository', reload)
    on('sync:reconnect', reload)
    // Connection-health auto-disable: reload to surface the new health columns.
    on('llm_repository.auto_disabled', () => {
      void actions.loadLlmRepositories(get().currentPage, get().pageSize)
    })
    void actions.loadLlmRepositories()
    // Auth-bootstrap race: this store's init runs on first access, which can be
    // BEFORE /auth/me populates permissions. The load above then bails on its
    // hasPermissionNow gate, and — with no post-auth re-trigger — the list stays
    // empty (API-seeded rows never appear on the settings page, and the
    // model-download drawer's repository picker renders no options). Re-fire the
    // self-gating load the moment LlmRepositoriesRead becomes available.
    watch(
      useAuthStore,
      () => hasPermissionNow(Permissions.LlmRepositoriesRead),
      (canRead, prev) => {
        if (canRead && !prev) void actions.loadLlmRepositories()
      },
    )
  },
})
export const LlmRepository = registerLazyStore(LlmRepositoryDef)
export const useLlmRepositoryStore = LlmRepositoryDef.store
