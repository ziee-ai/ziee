import { ApiClient } from '@/api-client'
import type { ConversationSummary } from '@/api-client/types'
import { defineStore } from '@/core/store-kit'

/** Single-entry read-model for the active conversation's branch summary
 *  (single-entry, not a Map — rotates on switch to avoid cross-user staleness). */
export const ConversationSummarization = defineStore('ConversationSummarization', {
  immer: true,
  state: {
    current: null as { conversationId: string; summary: ConversationSummary | null } | null,
    requestedConversationId: null as string | null,
    loading: false,
    error: null as string | null,
  },
  actions: (set, get) => ({
    loadForConversation: async (conversationId: string) => {
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
    },
    clear: () =>
      set(s => {
        s.current = null
        s.requestedConversationId = null
        s.loading = false
        s.error = null
      }),
  }),
})

export const useConversationSummarizationStore = ConversationSummarization.store
