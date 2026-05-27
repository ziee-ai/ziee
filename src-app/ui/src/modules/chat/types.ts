import type { StoreProxy } from '@/core/stores'
import type { useChatStore } from '@/modules/chat/core/stores/Chat.store'
import type { useChatHistoryStore } from '@/modules/chat/stores/ChatHistory.store'
import type { SidebarWidgetItem } from '@/modules/layouts/app-layout/types'

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
    Chat: StoreProxy<
      ReturnType<typeof useChatStore.getState> & ChatExtensionStores
    >
    ChatHistory: StoreProxy<ReturnType<typeof useChatHistoryStore.getState>>
  }
}

/**
 * Slot that other modules use to render content trailing the chat
 * header title (next to the TitleEditor in ConversationPage). The
 * Projects module uses this for `ConversationProjectChip` so the
 * chat module doesn't have a compile-time import on projects (closes
 * audit N11).
 *
 * Slot items render in `order` ascending, rightmost-first visually.
 * Use SidebarWidgetItem's shape so any module can register without
 * a new slot type.
 */
declare module '@/core/module-system/types' {
  interface Slots {
    chatConversationHeaderTrailing: SidebarWidgetItem[]
  }
}
