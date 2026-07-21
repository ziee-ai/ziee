import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { userAssistantsState, type UserAssistantsState } from './state'
import type { Actions } from './actions.gen'

const UserAssistantsDef = defineStore<UserAssistantsState, Actions>('UserAssistants', {
  immer: true,
  state: userAssistantsState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, actions }) => {
    // Reload the current page on any local mutation so pagination stays consistent.
    const reloadCurrent = () => void actions.loadUserAssistants()
    on('assistant.created', reloadCurrent)
    on('assistant.updated', reloadCurrent)
    on('assistant.deleted', reloadCurrent)
    // Remote sync: self-gate (reconnect fires for every store regardless of perms).
    const reload = () => {
      void actions.loadUserAssistants()
    }
    on('sync:assistant', reload)
    on('sync:reconnect', reload)
    void actions.loadUserAssistants()
  },
})
export const UserAssistants = registerLazyStore(UserAssistantsDef)
export const useUserAssistantsStore = UserAssistantsDef.store
