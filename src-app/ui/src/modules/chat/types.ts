import type { StoreProxy } from '@/core/stores'
import type { useChatLlmProviderStore } from './core/stores/LlmProvider.store'
import type { useChatStore } from './core/stores/Chat.store'
import type { useChatHistoryStore } from './stores/ChatHistory.store'

/**
 * Chat Extension Stores Interface
 * Each extension augments this interface with its own store type
 * This allows extensions to declare their types in their own files
 */
export interface ChatExtensionStores {
  // Extensions will augment this interface via declaration merging
}

declare module '@/core/stores' {
  interface RegisteredStores {
    ChatLlmProvider: StoreProxy<
      ReturnType<typeof useChatLlmProviderStore.getState>
    >
    Chat: StoreProxy<
      ReturnType<typeof useChatStore.getState> & ChatExtensionStores
    >
    ChatHistory: StoreProxy<ReturnType<typeof useChatHistoryStore.getState>>
  }
}
