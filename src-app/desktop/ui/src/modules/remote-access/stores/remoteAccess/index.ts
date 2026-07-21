import {
  defineStore,
  registerLazyStore,
  type FullStoreState,
  type DispatchersFromTypeMap,
} from '@ziee/framework/store-kit'
import type { StoreProxy } from '@ziee/framework/stores'
import { remoteAccessState, type RemoteAccessState } from './state'
import type { Actions } from './actions.gen'

// Re-export types so consumers that imported from the old store path still work.
export type { TunnelStateKind } from '@/api-client/types'
export type { RemoteAccessStatus, MagicLink } from './types'
export { remoteAccessState, type RemoteAccessState } from './state'

// Declare module augmentation so the store appears on RegisteredStores.
// The store's type must include state + all lazy dispatchers (FullStoreState).
declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    RemoteAccess: StoreProxy<
      FullStoreState<RemoteAccessState, DispatchersFromTypeMap<Actions>>
    >
  }
}

const RemoteAccessDef = defineStore<RemoteAccessState, Actions>('RemoteAccess', {
  immer: true,
  state: remoteAccessState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ actions }) => {
    // Eager-load so the settings page renders with real data on first mount.
    void actions.loadStatus()
  },
})

export const RemoteAccess = registerLazyStore(RemoteAccessDef)
export const useRemoteAccessStore = RemoteAccessDef.store
