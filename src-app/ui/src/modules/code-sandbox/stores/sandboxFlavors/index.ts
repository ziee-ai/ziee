import type { StoreProxy } from '@ziee/framework/stores'
import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { sandboxFlavorsState, type SandboxFlavorsState } from './state'
import type { Actions } from './actions.gen'

const SandboxFlavorsDef = defineStore<SandboxFlavorsState, Actions>('SandboxFlavors', {
  immer: true,
  state: sandboxFlavorsState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ actions }) => {
    void actions.load()
  },
})
export const SandboxFlavors = registerLazyStore(SandboxFlavorsDef)
export const useSandboxFlavorsStore = SandboxFlavorsDef.store

declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    SandboxFlavors: StoreProxy<ReturnType<typeof SandboxFlavorsDef.store.getState>>
  }
}
