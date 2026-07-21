import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { runtimeDeleteConfirmState, type RuntimeDeleteConfirmState } from './state'
import type { Actions } from './actions.gen'

const RuntimeDeleteConfirmDef = defineStore<RuntimeDeleteConfirmState, Actions>('RuntimeDeleteConfirm', {
  immer: true,
  state: runtimeDeleteConfirmState,
  actions: import.meta.glob('./actions/*.ts'),
})

export const RuntimeDeleteConfirm = registerLazyStore(RuntimeDeleteConfirmDef)
export const useRuntimeDeleteConfirmStore = RuntimeDeleteConfirmDef.store
