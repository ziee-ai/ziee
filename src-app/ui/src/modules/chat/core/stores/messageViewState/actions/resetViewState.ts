import type { MessageViewStateSet } from '../state'
import { emptyViewMaps } from '@/modules/chat/core/stores/messageViewState.helpers'

export default (set: MessageViewStateSet) => {
  /**
   * Drop ephemeral view state on conversation switch / store teardown
   * (ITEM-6, DEC-4).
   *
   * With `messageIds` (split-safe, ITEM-21): drop only THOSE messages'
   * collapse entries, so closing/switching one split pane never clobbers
   * another pane's still-open conversation. Message ids are globally unique,
   * so this store legitimately holds several conversations' view state at
   * once. Inline-file entries (keyed by resource_link URI, not message id)
   * are left intact — harmless to keep (idempotent, bounded).
   *
   * Without `messageIds` (single-pane / global): full reset, unchanged.
   */
  return (messageIds?: string[]) =>
    set(d => {
      if (!messageIds) return emptyViewMaps()
      for (const id of messageIds) delete d.collapsed[id]
    })
}
