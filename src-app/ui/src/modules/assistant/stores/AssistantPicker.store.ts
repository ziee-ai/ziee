import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import { Permissions, type Assistant } from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'

/**
 * Assistant picker store — the user's per-chat-composer assistant
 * selection plus the cached list of available assistants used by the
 * picker. Lives at `Stores.AssistantPicker` (registered in
 * `modules/assistant/module.tsx`).
 *
 * Lifecycle:
 *   - `availableAssistants` lazy-loads once via `__init__` on first
 *     access (the chat composer fetches it when it mounts).
 *   - `selectedAssistantId` is scoped to the active chat conversation;
 *     the chat-extension's `initialize()` hook resets it on
 *     conversation change.
 */
interface AssistantPickerState {
  // Data
  /** Assistant selected in the active composer (or null for "no assistant"). */
  selectedAssistantId: string | null
  /** Cached list of assistants the user can pick from. */
  availableAssistants: Assistant[]

  // Loading / error
  loading: boolean
  error: string | null

  // Lifecycle hooks (consumed by createStoreProxy)
  __init__: {
    availableAssistants: () => Promise<void>
  }

  // Actions
  loadAssistants: () => Promise<void>
  selectAssistant: (assistantId: string) => void
  clearAssistant: () => void
  reset: () => void
}

export const useAssistantPickerStore = create<AssistantPickerState>()(
  subscribeWithSelector(
    immer((set, get) => ({
      // State
      selectedAssistantId: null,
      availableAssistants: [],
      loading: false,
      error: null,

      __init__: {
        availableAssistants: () => get().loadAssistants(),
      },

      loadAssistants: async () => {
        // Permission-gate the shell-eager-load fetch — the chat shell
        // accesses the picker regardless of route; without
        // assistants::read the API 403s.
        if (!hasPermissionNow(Permissions.AssistantsRead)) return

        // Only load if not already loaded.
        const state = get()
        if (state.availableAssistants.length > 0) return

        set(s => {
          s.loading = true
          s.error = null
        })

        try {
          // Cap at 100 per server-side `limit` constraint (10-assistant
          // F-03 closure); chat-side picker wants everything visible.
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
      },

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
       * `initialize()` subscriber when the active conversation changes
       * (replaces the per-conversation auto-scoping that
       * `createExtensionStore` used to provide implicitly).
       */
      reset: () => {
        set(s => {
          s.selectedAssistantId = null
        })
      },
    })),
  ),
)
