import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import type { ConversationSummary } from '@/api-client/types'

/**
 * Read-model for the active conversation's branch summary.
 *
 * Single-entry cache (NOT a Map) per the audit lesson from the
 * crashed-session redo: a Map keyed by conversation id caused
 * cross-user staleness (the previous user's summary leaked into the
 * next user's session) AND unbounded growth (every conversation ever
 * opened stayed cached). Single-entry rotates on conversation
 * switch.
 *
 * Loaded by `SummarizationStatusPill` (which is always mounted in the
 * chat toolbar): subscribes to `messages.size` + `conversation.id` and
 * calls `loadForConversation` on change. This rides cross-device
 * freshness on `sync:conversation` transitively — DO NOT move the
 * trigger elsewhere (audit lesson).
 */
interface ConversationSummarizationStore {
  current: { conversationId: string; summary: ConversationSummary | null } | null
  loading: boolean
  error: string | null

  loadForConversation: (conversationId: string) => Promise<void>
  clear: () => void
}

export const useConversationSummarizationStore =
  create<ConversationSummarizationStore>()(
    subscribeWithSelector(
      immer((set, get) => ({
        current: null,
        loading: false,
        error: null,

        loadForConversation: async (conversationId: string) => {
          set(s => {
            s.loading = true
            s.error = null
          })
          try {
            const summary =
              await ApiClient.Summarization.getConversationSummary({
                conversation_id: conversationId,
              })
            // Guard against a slow load winning the race against a
            // newer conversation switch.
            if (get().current?.conversationId !== conversationId) {
              const target = get().current
              if (!target || target.conversationId !== conversationId) {
                // Switched conversation while the request was in
                // flight; drop the result.
                set(s => {
                  s.loading = false
                })
                return
              }
            }
            set(s => {
              s.current = { conversationId, summary }
              s.loading = false
            })
          } catch (error) {
            set(s => {
              s.error =
                error instanceof Error
                  ? error.message
                  : 'Failed to load summary'
              s.loading = false
            })
          }
        },

        clear: () => {
          set(s => {
            s.current = null
            s.error = null
          })
        },
      })),
    ),
  )
