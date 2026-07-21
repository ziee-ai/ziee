import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { coreMemoryBlocksState, type CoreMemoryBlocksGet, type CoreMemoryBlocksState } from './state'
import type { Actions } from './actions.gen'

const CoreMemoryBlocksDef = defineStore<CoreMemoryBlocksState, Actions>('CoreMemoryBlocks', {
  immer: true,
  state: coreMemoryBlocksState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, get, actions }) => {
    // Refresh open editors on reconnect / core-memory sync. load() is
    // permission-gated internally (memory::core::read).
    const get$: CoreMemoryBlocksGet = get as CoreMemoryBlocksGet
    const reloadAll = () => {
      Object.keys(get$().blocksByAssistant).forEach(id => void actions.load(id))
    }
    on('sync:assistant_core_memory', reloadAll)
    on('sync:reconnect', reloadAll)
  },
})

export const CoreMemoryBlocks = registerLazyStore(CoreMemoryBlocksDef)
export const useCoreMemoryBlocksStore = CoreMemoryBlocksDef.store
