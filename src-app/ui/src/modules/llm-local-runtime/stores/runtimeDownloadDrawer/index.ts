import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { runtimeDownloadDrawerState, type RuntimeDownloadDrawerState } from './state'
import type { Actions } from './actions.gen'

const RuntimeDownloadDrawerDef = defineStore<RuntimeDownloadDrawerState, Actions>('RuntimeDownloadDrawer', {
  immer: true,
  state: runtimeDownloadDrawerState,
  actions: import.meta.glob('./actions/*.ts'),
})
export const RuntimeDownloadDrawer = registerLazyStore(RuntimeDownloadDrawerDef)
export const useRuntimeDownloadDrawerStore = RuntimeDownloadDrawerDef.store
