import { createExtensionStore } from '@/modules/chat/core/extensions'
import { ApiClient } from '@/api-client'
import type { Assistant } from '@/api-client/types'

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
      // Only load if not already loaded
      const state = get()
      if (state.availableAssistants.length > 0) return

      set(state => {
        state.loading = true
        state.error = null
      })

      try {
        const response = await ApiClient.Assistant.list({})
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
