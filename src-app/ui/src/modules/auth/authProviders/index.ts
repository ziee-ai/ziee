import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { authProvidersState, type AuthProvidersState } from './state'
import type { Actions } from './actions.gen'

const AuthProvidersDef = defineStore<AuthProvidersState, Actions>('AuthProviders', {
  immer: true,
  state: authProvidersState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ actions }) => {
    void actions.loadProviders()
  },
})
export const AuthProviders = registerLazyStore(AuthProvidersDef)
export const useAuthProvidersStore = AuthProvidersDef.store
