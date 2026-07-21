import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { memoriesState, type MemoriesState } from './state'
import type { Actions } from './actions.gen'

const MemoriesDef = defineStore<MemoriesState, Actions>('Memories', {
  immer: true,
  state: memoriesState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, actions }) => {
    // Paginated list; load() reloads the current page (surfacing remote
    // creates/edits/deletes on it; a bulk-clear arrives as a nil-id Delete).
    // load() is permission-gated internally.
    const reload = () => void actions.load()
    on('sync:memory', reload)
    on('sync:reconnect', reload)
    void actions.load()
  },
})
export const Memories = registerLazyStore(MemoriesDef)
export const useMemoriesStore = MemoriesDef.store
