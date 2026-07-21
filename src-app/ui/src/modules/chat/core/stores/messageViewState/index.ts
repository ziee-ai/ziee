import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { messageViewStateState, type MessageViewStateState } from './state'
import type { Actions } from './actions.gen'

const MessageViewStateDef = defineStore<MessageViewStateState, Actions>('MessageViewState', {
  immer: true,
  state: messageViewStateState,
  actions: import.meta.glob('./actions/*.ts'),
})
export const MessageViewState = registerLazyStore(MessageViewStateDef)
export const useMessageViewStateStore = MessageViewStateDef.store

/** Full state (+ actions + lifecycle) — for scoped `useMessageViewStateStore`
 *  selectors that read a single keyed entry (avoids whole-map re-render). */
export type MessageViewFullState = ReturnType<typeof useMessageViewStateStore.getState>
