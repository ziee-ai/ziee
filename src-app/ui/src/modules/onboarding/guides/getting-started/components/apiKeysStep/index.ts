import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { apiKeysStepState, type ApiKeysStepState } from './state'
import type { Actions } from './actions.gen'

const ApiKeysStepDef = defineStore<ApiKeysStepState, Actions>('ApiKeysStep', {
  immer: true,
  state: apiKeysStepState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ actions }) => {
    void actions.loadProviders()
  },
})

export const ApiKeysStep = registerLazyStore(ApiKeysStepDef)
export const useApiKeysStepStore = ApiKeysStepDef.store

// Raw store for direct access (Stores proxy uses this).
export { ApiKeysStepDef }
