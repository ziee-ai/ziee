import { ApiClient } from '@/api-client'
import { type Assistant, Permissions } from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'

/**
 * Assistant picker store — the user's per-chat-composer assistant selection plus
 * the cached list of available assistants. Lives at `Stores.AssistantPicker`.
 * `availableAssistants` lazy-loads once on first access; `selectedAssistantId`
 * is reset per conversation by the chat-extension's `initialize()` hook.
 */
export const AssistantPicker = defineStore('AssistantPicker', {
  immer: true,
  state: {
    /** Assistant selected in the active composer (or null for "no assistant"). */
    selectedAssistantId: null as string | null,
    /** Cached list of assistants the user can pick from. */
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
      selectAssistant: (assistantId: string) => {
        set(s => {
          s.selectedAssistantId = assistantId
        })
      },
      clearAssistant: () => {
        set(s => {
          s.selectedAssistantId = null
        })
      },
      /**
       * Reset per-conversation state. Called from the chat-extension's
       * `initialize()` subscriber when the active conversation changes (replaces
       * the per-conversation auto-scoping createExtensionStore used to provide).
       */
      reset: () => {
        set(s => {
          s.selectedAssistantId = null
        })
      },
    }
  },
  init: ({ on, actions }) => {
    // Keep the cached picker list fresh on remote assistant create/edit/delete
    // (or reconnect). Self-gated on assistants::read (no-403 reconnect rule).
    const reload = () => {
      if (!hasPermissionNow(Permissions.AssistantsRead)) return
      void actions.loadAssistants(true)
    }
    on('sync:assistant', reload)
    on('sync:reconnect', reload)
    void actions.loadAssistants()
  },
})

export const useAssistantPickerStore = AssistantPicker.store
