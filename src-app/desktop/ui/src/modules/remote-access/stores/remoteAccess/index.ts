import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { type StoreProxy } from '@ziee/framework/stores'
import { remoteAccessState, type RemoteAccessData, type RemoteAccessState } from './state'
import type { Actions } from './actions.gen'

export type { TunnelStateKind, RemoteAccessStatus, MagicLink } from './state'

/**
 * Remote Access store — folder-glob lazy-store pattern (`state.ts` +
 * `actions/*.ts` + this index). Wraps the typed `ApiClient.RemoteAccess` +
 * `ApiClient.Auth` methods. Magic-link token rotation lives here: the page
 * rotates `issueMagicLink()` every 4 minutes (1 min before the 5-min token
 * expires) via a `rotationTimer` held in state. Actions call siblings through
 * `get()` (e.g. a mutation → `get().loadStatus()`), which resolves the lazy
 * dispatcher at call time — no factory import needed. The baked-in auto-warm
 * preloads every action chunk on init, so the sync timer-managers
 * (start/stopMagicLinkRotation) are hot by the time init / cleanup fire them.
 */
declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    RemoteAccess: StoreProxy<RemoteAccessState>
  }
}

// Module-registered (via `stores:` in module.tsx) — NOT `registerLazyStore` —
// preserving the original registration mechanism.
//
// EAGER glob: `startMagicLinkRotation()` / `stopMagicLinkRotation()` are
// SYNCHRONOUS timer-managers whose effect (set/clear `rotationTimer`) is relied
// upon synchronously (a caller — and the unit test — reads `rotationTimer`
// immediately after). A lazy dispatcher would defer that behind a dynamic
// import, so these load eagerly. The async actions stay async.
const RemoteAccessDef = defineStore<RemoteAccessData, Actions>('RemoteAccess', {
  immer: true,
  state: remoteAccessState,
  actions: import.meta.glob('./actions/*.ts', { eager: true }),
  init: ({ actions, onCleanup }) => {
    // Eager-load so the settings page renders with real data on first mount.
    void actions.loadStatus()
    onCleanup(() => actions.stopMagicLinkRotation())
  },
})

export const useRemoteAccessStore = RemoteAccessDef.store
export const RemoteAccess = registerLazyStore(RemoteAccessDef)
