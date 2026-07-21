import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { conversationHostMountsState, type ConversationHostMountsState } from './state'
import type { Actions } from './actions.gen'

// Re-export the state type so existing consumers that import it are satisfied.
export type { ConversationHostMountsState } from './state'

const ConversationHostMountsDef = defineStore<ConversationHostMountsState, Actions>('ConversationHostMounts', {
  immer: true,
  state: conversationHostMountsState,
  actions: import.meta.glob('./actions/*.ts'),
})
export const ConversationHostMounts = registerLazyStore(ConversationHostMountsDef)
export const useConversationHostMountsStore = ConversationHostMountsDef.store

// Keep the legacy module-augmentation declaration so the Stores proxy is typed.
declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    ConversationHostMounts: import('@ziee/framework/stores').StoreProxy<
      ReturnType<typeof useConversationHostMountsStore.getState>
    >
  }
}
