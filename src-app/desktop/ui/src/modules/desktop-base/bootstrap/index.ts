import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { bootstrapState, type BootstrapState, type BootstrapStatus } from './state'
import type { Actions } from './actions.gen'

const BootstrapDef = defineStore<BootstrapState, Actions>('Bootstrap', {
  immer: true,
  state: bootstrapState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ actions, onCleanup }) => {
    onCleanup(() => void actions.reset())
  },
})
export const Bootstrap = registerLazyStore(BootstrapDef)
export const useBootstrapStore = BootstrapDef.store
export type { BootstrapStatus }
