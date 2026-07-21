import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { knowledgeBaseComposerState, type KnowledgeBaseComposerState } from './state'
import type { Actions } from './actions.gen'

const KnowledgeBaseComposerDef = defineStore<KnowledgeBaseComposerState, Actions>('KnowledgeBaseComposer', {
  immer: true,
  state: knowledgeBaseComposerState,
  actions: import.meta.glob('./actions/*.ts'),
})

export const KnowledgeBaseComposer = registerLazyStore(KnowledgeBaseComposerDef)
export const useKnowledgeBaseComposerStore = KnowledgeBaseComposerDef.store
