import { ApiClient } from '@/api-client'
import { type Assistant, Permissions } from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@/core/store-kit'

/**
 * The composer key for a not-yet-created (new-chat) conversation's assistant
 * selection. Mirrors ModelPicker's NEW_CHAT_MODEL_KEY (ITEM-5).
 */
export const NEW_CHAT_ASSISTANT_KEY = '__new_chat__'

/**
 * The new-chat assistant key for a pane (ITEM-37) — a split pane gets its own
 * suffixed key so two new-chat panes don't share one assistant; a null paneId
 * (single-pane) keeps the bare `NEW_CHAT_ASSISTANT_KEY` (byte-identical).
 */
export const newChatAssistantKey = (
  paneId: string | null | undefined,
): string =>
  paneId ? `${NEW_CHAT_ASSISTANT_KEY}:${paneId}` : NEW_CHAT_ASSISTANT_KEY

/**
 * Assistant picker store — the user's per-chat-composer assistant selection plus
 * the cached list of available assistants. Lives at `Stores.AssistantPicker`.
 * `availableAssistants` is a GLOBAL catalog (lazy-loads once on first access).
 * The SELECTION is PER-CONVERSATION (`selectedByConversation`, keyed by
 * conversation id or `NEW_CHAT_ASSISTANT_KEY`) so two split panes each keep
 * their own assistant (ITEM-5). Per-conversation keying makes the old
 * reset-on-conversation-switch unnecessary — a conversation with no entry simply
 * has no assistant.
 */
export const AssistantPicker = defineStore('AssistantPicker', {
  immer: true,
  state: {
    /** Selected assistant id per conversation key (value null = "no assistant"). */
    selectedByConversation: {} as Record<string, string | null>,
    /** Cached list of assistants the user can pick from (GLOBAL catalog). */
    availableAssistants: [] as Assistant[],
    loading: false,
    error: null as string | null,
  },
  actions: (set, get) => {
    const loadAssistants = async (force = false) => {
      // Permission-gate the shell-eager-load fetch — the chat shell accesses the
      // picker regardless of route; without assistants::read the API 403s.
      if (!hasPermissionNow(Permissions.AssistantsRead)) return
      // Only load once unless a sync event forces a refresh.
      if (!force && get().availableAssistants.length > 0) return
      set(s => {
        s.loading = true
        s.error = null
      })
      try {
        // Cap at 100 per server-side `limit`; the picker wants everything visible.
        const response = await ApiClient.Assistant.list({ page: 1, limit: 100 })
        set(s => {
          s.availableAssistants = response.assistants
          s.loading = false
        })
      } catch (error: any) {
        set(s => {
          s.error = error.message || 'Failed to load assistants'
          s.loading = false
        })
      }
    }
    return {
      loadAssistants,
      selectAssistant: (key: string, assistantId: string) => {
        set(s => {
          s.selectedByConversation[key] = assistantId
        })
      },
      clearAssistant: (key: string) => {
        set(s => {
          s.selectedByConversation[key] = null
        })
      },
      /** The selected assistant id for a conversation key (null = none / unset). */
      getAssistantId: (key: string): string | null =>
        get().selectedByConversation[key] ?? null,
    }
  },
  init: ({ on, set, actions }) => {
    // Keep the cached picker list fresh on remote assistant create/edit/delete
    // (or reconnect). Self-gated on assistants::read (no-403 reconnect rule).
    const reload = () => {
      if (!hasPermissionNow(Permissions.AssistantsRead)) return
      void actions.loadAssistants(true)
    }
    on('sync:assistant', reload)
    on('sync:reconnect', reload)
    // Prune a deleted conversation's per-conversation assistant selection so the
    // `selectedByConversation` map doesn't grow unbounded / retain stale keys.
    on('sync:conversation', event => {
      if (event.data.action === 'delete') {
        set(state => {
          delete state.selectedByConversation[event.data.id]
        })
      }
    })
    void actions.loadAssistants()
  },
})

export const useAssistantPickerStore = AssistantPicker.store
