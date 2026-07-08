import { defineStore } from '@/core/store-kit'
import {
  emptyViewMaps,
  type InlineFileViewState,
  DEFAULT_INLINE_FILE_STATE,
} from '@/modules/chat/core/stores/messageViewState.helpers'

/**
 * MessageViewState — per-conversation, in-memory store for the ephemeral per-row
 * UI state that must SURVIVE the virtualizer unmounting/remounting a row
 * (message-scroll-stability, ITEM-6). See `messageViewState.helpers.ts` for the
 * why. Reset on conversation switch by `Chat.store` (mirrors how it clears the
 * `messages` window + `conversationStateCache`) — this holds state for the
 * currently-open conversation only (DEC-4). Not persisted to disk.
 *
 * Reads are proxy reads (`Stores.MessageViewState.collapsed[id]`); toggles are
 * rare user actions, so re-rendering the (few) collapsible rows on a change is
 * negligible and matches the existing find-highlight re-render-all.
 */
export const MessageViewState = defineStore('MessageViewState', {
  immer: true,
  state: {
    /** message id → collapsed (absent ⇒ default-collapsed). */
    collapsed: {} as Record<string, boolean>,
    /** resource_link URI → InlineFileViewState (absent ⇒ default). */
    files: {} as Record<string, InlineFileViewState>,
  },
  actions: set => {
    /** Ensure a file entry exists (seeded from defaults) before mutating it. */
    const ensureFile = (
      files: Record<string, InlineFileViewState>,
      key: string,
    ): InlineFileViewState => {
      if (!files[key]) files[key] = { ...DEFAULT_INLINE_FILE_STATE }
      return files[key]
    }
    return {
      /** Show-more toggle for a long message (ITEM-4). */
      setMessageCollapsed: (messageId: string, collapsed: boolean) =>
        set(d => {
          d.collapsed[messageId] = collapsed
        }),
      /** Inline-file chevron toggle (ITEM-5). */
      setFileCollapsed: (key: string, collapsed: boolean) =>
        set(d => {
          ensureFile(d.files, key).collapsed = collapsed
        }),
      /** Mark an inline file's body as having entered view once (ITEM-5). */
      markFileSeen: (key: string) =>
        set(d => {
          ensureFile(d.files, key).seen = true
        }),
      /** Persist a user-dragged inline-file body height in px (ITEM-3/5). */
      setFileHeight: (key: string, heightPx: number) =>
        set(d => {
          ensureFile(d.files, key).heightPx = heightPx
        }),
      /** Drop ALL view state — called on conversation switch (ITEM-6, DEC-4). */
      resetViewState: () => set(() => emptyViewMaps()),
    }
  },
})

export const useMessageViewStateStore = MessageViewState.store
