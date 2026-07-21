import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { knowledgeBasesState, type KnowledgeBasesState } from './state'
import type { Actions } from './actions.gen'

const KnowledgeBasesDef = defineStore<KnowledgeBasesState, Actions>('KnowledgeBases', {
  immer: true,
  state: knowledgeBasesState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, actions }) => {
    // Cross-device sync — `load` self-gates on the permission.
    const reload = () => void actions.load(true)
    on('sync:knowledge_base', reload)
    on('sync:reconnect', reload)
    void actions.load()
  },
})
export const KnowledgeBases = registerLazyStore(KnowledgeBasesDef)
export const useKnowledgeBasesStore = KnowledgeBasesDef.store
