import { createExtensionStore } from '@/modules/chat/core/extensions'
import { ApiClient } from '@/api-client'
import { Permissions, type Assistant } from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'

/**
 * Assistant extension store
 * Combines state and actions
 */
interface AssistantStore {
  // State
  /** Selected assistant ID for current conversation */
  selectedAssistantId: string | null
  /** List of available assistants (loaded once, shared globally) */
  availableAssistants: Assistant[]
  /** Loading state for assistant list */
  loading: boolean
  /** Error state for assistant list */
  error: string | null

  // Actions
  /** Load available assistants (called once globally) */
  loadAssistants: () => Promise<void>
  /** Select assistant for current conversation */
  selectAssistant: (assistantId: string) => void
  /** Clear selected assistant */
  clearAssistant: () => void
}

/**
 * Create assistant extension store
 * Independent Zustand store with full reactivity
 * Accessible via Stores.Chat.AssistantStore
 */
export const createAssistantStore = () =>
  createExtensionStore<AssistantStore>((set, get) => ({
    // State
    selectedAssistantId: null,
    availableAssistants: [],
    loading: false,
    error: null,

    // Actions
    /**
     * Load available assistants
     * Called once globally, not per-conversation
     */
    loadAssistants: async () => {
      // Permission-gate the shell-eager-load fetch (audit
      // follow-up): the chat shell loads the assistant picker
      // regardless of route. Without assistants::read it 403s.
      if (!hasPermissionNow(Permissions.AssistantsRead)) return

      // Only load if not already loaded
      const state = get()
      if (state.availableAssistants.length > 0) return

      set(state => {
        state.loading = true
        state.error = null
      })

      try {
        // 10-assistant F-03 closure made page + limit required on the
        // server side (was unbounded). Use a generous limit since the
        // chat-side assistant picker wants everything visible at once
        // anyway; the server caps `limit` at 100.
        const response = await ApiClient.Assistant.list({ page: 1, limit: 100 })
        set(state => {
          state.availableAssistants = response.assistants
          state.loading = false
        })
      } catch (error: any) {
        set(state => {
          state.error = error.message || 'Failed to load assistants'
          state.loading = false
        })
      }
    },

    /**
     * Select assistant for current conversation
     */
    selectAssistant: (assistantId: string) => {
      set(state => {
        state.selectedAssistantId = assistantId
      })
    },

    /**
     * Clear selected assistant
     */
    clearAssistant: () => {
      set(state => {
        state.selectedAssistantId = null
      })
    },

    /**
     * Lifecycle hooks for lazy loading
     */
    __init__: {
      // Lazy load assistants when first accessed
      availableAssistants: () => get().loadAssistants(),
    },
  }))

/**
 * Augment ChatExtensionStores with AssistantStore
 */
declare module '../../types' {
  interface ChatExtensionStores {
    AssistantStore: ReturnType<typeof createAssistantStore>
  }
}
