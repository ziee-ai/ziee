import type { StoreProxy } from '@/core/stores'
import type { useChatLlmProviderStore } from './core/stores/LlmProvider.store'
import type { useChatStore } from './core/stores/Chat.store'
import type { useChatHistoryStore } from './stores/ChatHistory.store'
import type { createAssistantStore } from './extensions/assistant/AssistantStore.store'
import type { createMcpStore } from './extensions/mcp/McpStore.store'

declare module '@/core/stores' {
  interface RegisteredStores {
    ChatLlmProvider: StoreProxy<
      ReturnType<typeof useChatLlmProviderStore.getState>
    >
    Chat: StoreProxy<
      ReturnType<typeof useChatStore.getState> & {
        // Extension stores injected at runtime
        AssistantStore: ReturnType<typeof createAssistantStore>
        McpStore: ReturnType<typeof createMcpStore>
      }
    >
    ChatHistory: StoreProxy<ReturnType<typeof useChatHistoryStore.getState>>
  }
}

export {}
