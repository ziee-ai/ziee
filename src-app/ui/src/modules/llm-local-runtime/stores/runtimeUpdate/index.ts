import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { runtimeUpdateState, type RuntimeUpdateState } from './state'
import type { Actions } from './actions.gen'

const RuntimeUpdateDef = defineStore<RuntimeUpdateState, Actions>('RuntimeUpdate', {
  immer: true,
  state: runtimeUpdateState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, get, actions }) => {
    // When a version is deleted or created, the cached updateChecks for its
    // engine still flag it as installed/not (stale). Re-run the check for every
    // engine that has a cached entry to rebuild against current DB state. Cheap:
    // only two engines.
    const refreshAllCached = () => {
      for (const engine of get().updateChecks.keys()) {
        actions.checkForUpdates(engine).catch(() => {})
      }
    }
    on('runtime_version.deleted', refreshAllCached)
    on('runtime_version.created', refreshAllCached)
  },
})
export const RuntimeUpdate = registerLazyStore(RuntimeUpdateDef)
export const useRuntimeUpdateStore = RuntimeUpdateDef.store

// Raw (non-proxy) store for gallery / direct setState needs
export const { store: RuntimeUpdateRaw } = RuntimeUpdateDef
