import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { userProviderKeysState, type UserProviderKeysState } from './state'
import type { Actions } from './actions.gen'

const UserProviderKeysDef = defineStore<UserProviderKeysState, Actions>('UserProviderKeys', {
  immer: true,
  state: userProviderKeysState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, set, actions }) => {
    // Cross-device sync: a key saved/removed on another device (or a missed
    // event across a dropped stream) invalidates this per-provider cache. Reset
    // `initialized` so the guarded loadKeys() actually refetches; loadKeys() is
    // permission-gated internally (profile::read).
    const reload = () => {
      set({ initialized: false })
      void actions.loadKeys()
    }
    on('sync:api_key', reload)
    on('sync:reconnect', reload)
  },
})
export const UserProviderKeys = registerLazyStore(UserProviderKeysDef)
export const useUserProviderKeysStore = UserProviderKeysDef.store
