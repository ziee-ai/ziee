import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { conversationSummarizationState, type ConversationSummarizationState } from './state'
import type { Actions } from './actions.gen'

const ConversationSummarizationDef = defineStore<ConversationSummarizationState, Actions>(
  'ConversationSummarization',
  {
    immer: true,
    state: conversationSummarizationState,
    actions: import.meta.glob('./actions/*.ts'),
  },
)
export const ConversationSummarization = registerLazyStore(ConversationSummarizationDef)
export const useConversationSummarizationStore = ConversationSummarizationDef.store
