import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { runtimeConfigState, type RuntimeConfigState } from './state'
import type { Actions } from './actions.gen'

const RuntimeConfigDef = defineStore<RuntimeConfigState, Actions>('RuntimeConfig', {
  immer: true,
  state: runtimeConfigState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, actions }) => {
    // Cross-device sync: reload the deployment-wide runtime settings (singleton)
    // on a remote change or after an SSE reconnect. loadSettings self-gates.
    const reload = () => void actions.loadSettings()
    on('sync:runtime_settings', reload)
    on('sync:reconnect', reload)
    void actions.loadSettings()
    void actions.loadGpu()
  },
})
export const RuntimeConfig = registerLazyStore(RuntimeConfigDef)
export const useRuntimeConfigStore = RuntimeConfigDef.store

// Raw (non-proxy) store for gallery / direct setState needs
export const { store: RuntimeConfigRaw } = RuntimeConfigDef
