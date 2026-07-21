import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { citationsState, type CitationsState } from './state'
import type { Actions } from './actions.gen'

const CitationsDef = defineStore<CitationsState, Actions>('Citations', {
  immer: true,
  state: citationsState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, actions }) => {
    // Notify-and-refetch: the backend emits `BibliographyEntry` (owner-scoped)
    // on add/import/delete/attach; refetch on reconnect too.
    const reload = () => void actions.load()
    on('sync:bibliography_entry', reload)
    on('sync:reconnect', reload)
    void actions.load()
  },
})
export const Citations = registerLazyStore(CitationsDef)
export const useCitationsStore = CitationsDef.store
