import { ApiClient } from '@/api-client'
import type { ConversationSummarizationGet, ConversationSummarizationSet } from '../state'

export default (set: ConversationSummarizationSet, get: ConversationSummarizationGet) =>
  async (conversationId: string) => {
    set(s => {
      // Drop stale `current` when loading a different conversation.
      if (s.current && s.current.conversationId !== conversationId) s.current = null
      s.requestedConversationId = conversationId
      s.loading = true
      s.error = null
    })
    try {
      const summary = await ApiClient.Summarization.getConversationSummary({ id: conversationId })
      // Switched while in flight → drop.
      if (get().requestedConversationId !== conversationId) {
        set(s => { s.loading = false })
        return
      }
      set(s => {
        s.current = { conversationId, summary }
        s.loading = false
      })
    } catch (error) {
      if (get().requestedConversationId !== conversationId) {
        set(s => { s.loading = false })
        return
      }
      set(s => {
        s.error = error instanceof Error ? error.message : 'Failed to load summary'
        s.loading = false
      })
    }
  }
