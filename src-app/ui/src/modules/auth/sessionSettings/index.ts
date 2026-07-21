import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { sessionSettingsState, type SessionSettingsState } from './state'
import { hasPermissionNow } from '@/core/permissions'
import { Permissions } from '@/api-client/permissions'
import type { Actions } from './actions.gen'

const SessionSettingsDef = defineStore<SessionSettingsState, Actions>('SessionSettings', {
  immer: true,
  state: sessionSettingsState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, actions }) => {
    const reload = () => {
      if (!hasPermissionNow(Permissions.SessionSettingsRead)) return
      void actions.load()
    }
    on('sync:session_settings', reload)
    on('sync:reconnect', reload)
    if (hasPermissionNow(Permissions.SessionSettingsRead)) void actions.load()
  },
})

// The raw Zustand store for gallery setup that needs direct setState.
export const SessionSettingsStore = SessionSettingsDef.store

export const SessionSettings = registerLazyStore(SessionSettingsDef)
export const useSessionSettingsStore = SessionSettingsDef.store
export type { SessionSettingsState }
