import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { webSearchAdminInitialState, type WebSearchAdminState } from './state'
import type { Actions } from './actions.gen'

/** WebSearchAdmin — folder-pattern store: state.ts + index.ts + actions/. Actions
 *  auto-register from `./actions/*.ts` by filename (no hand-written map). */
const WebSearchAdminDef = defineStore<WebSearchAdminState, Actions>('WebSearchAdmin', {
  immer: true,
  state: webSearchAdminInitialState,
  actions: import.meta.glob('./actions/*.ts'),
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
export const useWebSearchAdminStore = WebSearchAdminDef.store

export const WebSearchAdmin = registerLazyStore(WebSearchAdminDef)
