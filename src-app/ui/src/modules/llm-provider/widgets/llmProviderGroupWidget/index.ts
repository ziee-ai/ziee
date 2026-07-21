import { defineLocalStore } from '@ziee/framework/store-kit'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import {
  llmProviderGroupWidgetState,
  type LlmProviderGroupWidgetState,
} from './state'
import type { Actions } from './actions.gen'

/**
 * PRIVATE, per-widget store (one instance per group row) — folder-glob lazy-store
 * pattern (`state.ts` + `actions/*.ts` + this index). Each mounted widget owns
 * just ITS group's providers; `init` fetches on MOUNT (so it's populated after a
 * reload with no consumer-side effect) and its event listeners auto-unsubscribe
 * on UNMOUNT. Actions auto-register from `./actions/*.ts` by filename.
 */
export const LlmProviderGroupWidgetStore = defineLocalStore<
  LlmProviderGroupWidgetState,
  Actions
>({
  immer: true,
  state: llmProviderGroupWidgetState,
  actions: import.meta.glob('./actions/*.ts'),

  // Runs on MOUNT; every `on(...)` auto-unsubscribes on UNMOUNT.
  init: ({ on, get, set, actions }) => {
    // `GET /api/groups/{id}/providers` requires llm_providers::read (not
    // user-held); guard the eager mount fetch so a groups-admin without it
    // (viewing the user-groups page) doesn't 403.
    if (hasPermissionNow(Permissions.LlmProvidersRead)) {
      void actions.load()
    }

    // Real-time updates, scoped to THIS instance's group.
    on('llm_provider.group_providers_changed', async event => {
      if (event.data.groupId !== get().groupId) return
      await actions.load(true)
    })
    on('llm_provider.created', () => {
      void actions.load(true)
    })
    on('llm_provider.updated', event => {
      set(d => {
        const i = d.providers.findIndex(p => p.id === event.data.provider.id)
        if (i !== -1) d.providers[i] = event.data.provider
      })
    })
    on('llm_provider.deleted', event => {
      set(d => {
        d.providers = d.providers.filter(p => p.id !== event.data.providerId)
      })
    })
  },
})
