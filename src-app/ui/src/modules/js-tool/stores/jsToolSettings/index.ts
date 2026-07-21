import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { jsToolSettingsState, type JsToolSettingsState } from './state'
import type { Actions } from './actions.gen'

const JsToolSettingsDef = defineStore<JsToolSettingsState, Actions>('JsToolSettings', {
  immer: true,
  state: jsToolSettingsState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, actions }) => {
    // Singleton row. Refetch on a remote change or SSE reconnect. Self-gate the
    // refetch (no-403 reconnect rule): sync:reconnect fires for every store
    // regardless of audience, so a user without settings-read must not refetch.
    // The perm MUST equal the GET's read-perm.
    const reload = () => {
      if (!hasPermissionNow(Permissions.JsToolSettingsRead)) return
      void actions.loadSettings()
    }
    on('sync:js_tool_settings', reload)
    on('sync:reconnect', reload)
    void actions.loadSettings()
  },
})
export const JsToolSettings = registerLazyStore(JsToolSettingsDef)
export const useJsToolSettingsStore = JsToolSettingsDef.store
