import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { memorySettingsState, type MemorySettingsState } from './state'
import type { Actions } from './actions.gen'

const MemorySettingsDef = defineStore<MemorySettingsState, Actions>(
  'MemorySettings',
  {
    immer: true,
    state: memorySettingsState,
    actions: import.meta.glob('./actions/*.ts'),
    init: ({ on, actions }) => {
      // Per-user singleton — refetch it. `load()` is permission-gated internally.
      const reload = () => void actions.load()
      on('sync:memory_settings', reload)
      on('sync:reconnect', reload)
      void actions.load()
    },
  },
)
export const MemorySettings = registerLazyStore(MemorySettingsDef)
export const useMemorySettingsStore = MemorySettingsDef.store
