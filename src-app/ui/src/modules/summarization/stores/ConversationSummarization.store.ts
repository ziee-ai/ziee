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
  // The conversation the most-recent `loadForConversation` was started for.
  // When a load resolves, we accept its result only if this still matches —
  // an older in-flight request whose user has already switched conversations
  // gets dropped instead of clobbering the new conversation's summary.
  requestedConversationId: string | null
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
        requestedConversationId: null,
        loading: false,
        error: null,

        loadForConversation: async (conversationId: string) => {
          set(s => {
            // Drop stale `current` when starting a load for a different
            // conversation — readers (e.g. SummaryBoundaryMarker) only
            // key on `summary.summarized_up_to_id === message.id`, so a
            // surviving prior-conversation `current` could briefly
            // render against the new conversation's messages.
            if (s.current && s.current.conversationId !== conversationId) {
              s.current = null
            }
            s.requestedConversationId = conversationId
            s.loading = true
            s.error = null
          })
          try {
            const summary =
              await ApiClient.Summarization.getConversationSummary({
                id: conversationId,
              })
            // Switched conversation while the request was in flight; drop
            // the result rather than clobber the new conversation.
            if (get().requestedConversationId !== conversationId) {
              set(s => {
                s.loading = false
              })
              return
            }
            set(s => {
              s.current = { conversationId, summary }
              s.loading = false
            })
          } catch (error) {
            // Same race-guard on the failure path — a stale error from
            // the prior conversation must not poison the new one.
            if (get().requestedConversationId !== conversationId) {
              set(s => {
                s.loading = false
              })
              return
            }
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
            s.requestedConversationId = null
            s.loading = false
            s.error = null
          })
        },
      })),
    ),
  )
