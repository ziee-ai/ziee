import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { tunnelAuthState, type TunnelAuthState } from './state'
import type { Actions } from './actions.gen'

const TunnelAuthDef = defineStore<TunnelAuthState, Actions>('TunnelAuth', {
  state: tunnelAuthState,
  actions: import.meta.glob('./actions/*.ts'),
})
export const TunnelAuth = registerLazyStore(TunnelAuthDef)
export const useTunnelAuthStore = TunnelAuthDef.store

export type { TunnelAuthState, TunnelAuthSet, TunnelAuthGet } from './state'
