import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { userLlmProvidersState, type UserLlmProvidersState } from './state'
import type { Actions } from './actions.gen'

const UserLlmProvidersDef = defineStore<UserLlmProvidersState, Actions>('UserLlmProviders', {
  immer: true,
  state: userLlmProvidersState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, actions }) => {
    on('llm_provider.created', () => void actions.load())
    on('llm_provider.updated', () => void actions.load())
    on('llm_provider.deleted', () => void actions.load())
    // Remote sync: an API key / provider / model changed on another device, or
    // we (re)connected. load() self-gates on UserLlmProvidersRead.
    const reload = () => void actions.load()
    on('sync:api_key', reload)
    on('sync:user_llm_provider', reload)
    on('sync:reconnect', reload)
    void actions.load()
  },
})
export const UserLlmProviders = registerLazyStore(UserLlmProvidersDef)
export const useUserLlmProvidersStore = UserLlmProvidersDef.store
