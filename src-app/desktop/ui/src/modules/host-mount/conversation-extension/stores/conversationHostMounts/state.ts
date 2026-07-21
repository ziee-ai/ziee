import type { StoreSet } from '@ziee/framework/store-kit'
import type { MountEntry } from '@/api-client/types'

export const conversationHostMountsState = {
  byConversation: {} as Record<string, MountEntry[]>,
  loading: false,
  saving: false,
  error: null as string | null,
}

export type ConversationHostMountsState = typeof conversationHostMountsState
export type ConversationHostMountsSet = StoreSet<ConversationHostMountsState>
export type ConversationHostMountsGet = () => ConversationHostMountsState
