import type { ConversationSummary } from '@/api-client/types'
import type { StoreSet } from '@ziee/framework/store-kit'

export const conversationSummarizationState = {
  current: null as { conversationId: string; summary: ConversationSummary | null } | null,
  requestedConversationId: null as string | null,
  loading: false,
  error: null as string | null,
}

export type ConversationSummarizationState = typeof conversationSummarizationState
export type ConversationSummarizationSet = StoreSet<ConversationSummarizationState>
export type ConversationSummarizationGet = () => ConversationSummarizationState
