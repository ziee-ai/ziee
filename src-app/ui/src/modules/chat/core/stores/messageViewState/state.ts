import type { StoreSet } from '@ziee/framework/store-kit'
import type { InlineFileViewState } from '@/modules/chat/core/stores/messageViewState.helpers'

export const messageViewStateState = {
  /** message id → collapsed (absent ⇒ default-collapsed). */
  collapsed: {} as Record<string, boolean>,
  /** resource_link URI → InlineFileViewState (absent ⇒ default). */
  files: {} as Record<string, InlineFileViewState>,
}

export type MessageViewStateState = typeof messageViewStateState
export type MessageViewStateSet = StoreSet<MessageViewStateState>
export type MessageViewStateGet = () => MessageViewStateState
