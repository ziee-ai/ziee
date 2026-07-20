import type { ChatSet, ChatInitialState, ChatState } from '@/modules/chat/core/stores/Chat.store'

export default (set: ChatSet, getRaw: () => ChatInitialState) => {
  const get = getRaw as unknown as () => ChatState
  return async () => {
      // Capture the edited message id BEFORE clearing so we can restore its
      // neighborhood (not just the tail) when it was scrolled up mid-history.
      const editedId = get().editingMessage?.id

      // Clear text input first
      ;(get() as any).TextStore?.clearText()

      // Clear editing state — extensions react via their subscribe handlers
      set({
        editingMessage: null,
        pendingBranchFromMessageId: null,
        pendingBranchForkLevel: null,
      })

      // Restore what was trimmed by startEditMessage. If the edited message sat
      // in the middle of a long (lazy-loaded) history, restore the window
      // CENTERED on it (around=) rather than snapping to the tail; fall back to
      // the tail if it can't be located on the active branch.
      const conversationId = get().conversation?.id
      if (!conversationId) return
      if (editedId) {
        const ok = await get().jumpToMessage(editedId)
        if (!ok) await get().loadMessages(conversationId)
      } else {
        await get().loadMessages(conversationId)
      }
    }
}
