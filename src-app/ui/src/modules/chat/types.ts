import type { StoreProxy } from '@ziee/framework/stores'
import type { useChatStore } from '@/modules/chat/core/stores/chat'
import type { useChatHistoryStore } from '@/modules/chat/stores/chatHistory'
import type { useMessageViewStateStore } from '@/modules/chat/core/stores/messageViewState'
import type { useSplitViewStore } from '@/modules/chat/core/stores/splitView'
import type { SidebarWidgetItem } from '@/modules/layouts/app-layout/types'

/**
 * Chat Extension Stores Interface
 * Each extension augments this interface with its own store type
 * This allows extensions to declare their types in their own files
 */
export interface ChatExtensionStores {
  // Extensions will augment this interface via declaration merging
}

declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    Chat: StoreProxy<
      ReturnType<typeof useChatStore.getState> & ChatExtensionStores
    >
    ChatHistory: StoreProxy<ReturnType<typeof useChatHistoryStore.getState>>
    MessageViewState: StoreProxy<
      ReturnType<typeof useMessageViewStateStore.getState>
    >
    SplitView: StoreProxy<ReturnType<typeof useSplitViewStore.getState>>
  }
}

/**
 * Slot that other modules use to render content trailing the chat
 * header title (next to the TitleEditor in ConversationPage).
 * Decoupled injection point — chat itself doesn't compile against
 * any consumer.
 *
 * Slot items render in `order` ascending, rightmost-first visually.
 * Use SidebarWidgetItem's shape so any module can register without
 * a new slot type.
 */
declare module '@ziee/framework/module-system/types' {
  interface Slots {
    chatConversationHeaderTrailing: SidebarWidgetItem[]
  }
}
