import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { type StoreProxy } from '@ziee/framework/stores'
import { hostMountPolicyState, type HostMountPolicyState } from './state'
import type { Actions } from './actions.gen'

const HostMountPolicyDef = defineStore<HostMountPolicyState, Actions>('HostMountPolicy', {
  immer: true,
  state: hostMountPolicyState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ actions }) => {
    void actions.loadPolicy()
  },
})
export const HostMountPolicy = registerLazyStore(HostMountPolicyDef)
export const useHostMountPolicyStore = HostMountPolicyDef.store

// Keep the legacy module-augmentation declaration so the Stores proxy is typed.
declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    HostMountPolicy: StoreProxy<
      ReturnType<typeof useHostMountPolicyStore.getState>
    >
  }
}
