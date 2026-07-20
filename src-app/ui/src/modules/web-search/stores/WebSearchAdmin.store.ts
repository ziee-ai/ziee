import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'
import {
  webSearchAdminInitialState,
  type WebSearchAdminState,
} from './webSearchAdmin/types'

/**
 * WebSearchAdmin — PROOF of the per-action lazy-loading store pattern.
 *
 * The state lives here (eager, tiny, always present → `Stores.WebSearchAdmin.x`
 * never `undefined`). Every ACTION lives in its own file under
 * `webSearchAdmin/actions/` and is loaded as its OWN chunk on first call (or
 * `preload()`), NOT baked into the entry chunk. All lazy actions are async.
 *
 * Inter-action calls (`load` → loadSettings/loadProviders) reach the sibling
 * dispatchers through the runtime state (`get()`), which carries them.
 */
export const WebSearchAdmin = defineStore('WebSearchAdmin', {
  immer: true,
  state: webSearchAdminInitialState,
  lazyActions: {
    loadSettings: () => import('./webSearchAdmin/actions/loadSettings'),
    loadProviders: () => import('./webSearchAdmin/actions/loadProviders'),
    updateSettings: () => import('./webSearchAdmin/actions/updateSettings'),
    updateProvider: () => import('./webSearchAdmin/actions/updateProvider'),
  },
  // A trivial composite that just fans out to two lazy loaders in parallel — a
  // synchronous wrapper (no own chunk needed); it calls the dispatchers off the
  // live state so each loads its own chunk.
  actions: (_set, get) => ({
    load: async (): Promise<void> => {
      const s = get() as WebSearchAdminState & {
        loadSettings: () => Promise<void>
        loadProviders: () => Promise<void>
      }
      await Promise.all([s.loadSettings(), s.loadProviders()])
    },
  }),
  // Loaders hit `web_search::admin::read`-gated endpoints. Self-gate so
  // non-admins never generate 403s (incl. on the audience-agnostic reconnect).
  init: ({ on, actions }) => {
    const reload = () => {
      if (!hasPermissionNow(Permissions.WebSearchAdminRead)) return
      void actions.loadSettings()
      void actions.loadProviders()
    }
    on('sync:web_search_settings', reload)
    on('sync:reconnect', reload)
    reload()
  },
})

export const useWebSearchAdminStore = WebSearchAdmin.store
