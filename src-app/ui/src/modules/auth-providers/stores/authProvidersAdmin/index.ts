import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { authProvidersAdminState, type AuthProvidersAdminState } from './state'
import type { Actions } from './actions.gen'

const AuthProvidersAdminDef = defineStore<AuthProvidersAdminState, Actions>(
  'AuthProvidersAdmin',
  {
    immer: true,
    state: authProvidersAdminState,
    actions: import.meta.glob('./actions/*.ts'),
    init: ({ on, set, actions }) => {
      // In-process created/updated/deleted from local actions.
      on('auth_provider.created', event => {
        set(state => {
          state.providers.push(event.data.provider)
          state.providers.sort((a, b) => a.name.localeCompare(b.name))
        })
      })
      on('auth_provider.updated', event => {
        set(state => {
          const idx = state.providers.findIndex(p => p.id === event.data.provider.id)
          if (idx >= 0) state.providers[idx] = event.data.provider
        })
      })
      on('auth_provider.deleted', event => {
        set(state => {
          state.providers = state.providers.filter(p => p.id !== event.data.providerId)
          state.testingIds.delete(event.data.providerId)
        })
      })
      // Auto-disable: the backend (or another tab) flipped a row to enabled=false
      // because its probe failed. Reload so Switch + Alert reflect canonical state.
      on('auth_provider.auto_disabled', () => void actions.loadProviders())
      // Cross-device sync. loadProviders self-guards against in-flight loads.
      const reload = () => void actions.loadProviders()
      on('sync:auth_provider', reload)
      on('sync:reconnect', reload)
      void actions.loadProviders()
    },
  },
)
export const AuthProvidersAdmin = registerLazyStore(AuthProvidersAdminDef)
export const useAuthProvidersAdminStore = AuthProvidersAdminDef.store
