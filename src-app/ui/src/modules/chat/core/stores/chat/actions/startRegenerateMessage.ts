
import type { ChatSet, ChatInitialState, ChatState } from '@/modules/chat/core/stores/Chat.store'

export default (set: ChatSet, getRaw: () => ChatInitialState) => {
  const get = getRaw as unknown as () => ChatState
  return async (assistantMessageId: string) => {
      const sorted = [...get().messages.values()].sort(
        (a, b) =>
          new Date(a.created_at).getTime() - new Date(b.created_at).getTime(),
      )

      const currentIndex = sorted.findIndex(m => m.id === assistantMessageId)
      if (currentIndex <= 0) return

      let precedingUserMsg = null
      for (let i = currentIndex - 1; i >= 0; i--) {
        if (sorted[i].role === 'user') {
          precedingUserMsg = sorted[i]
          break
        }
      }

      if (!precedingUserMsg) return

      const userText = (() => {
        for (const content of precedingUserMsg.contents) {
          const data = content.content as any
          if (data?.type === 'text' && typeof data.text === 'string') {
            return data.text
          }
        }
        return ''
      })()

      // Fan out content-block restoration to every extension —
      // each filters by its own content_type and rehydrates its
      // store accordingly (file restores `file_attachment` blocks
      // into its selectedFiles buffer; future extensions can do the
      // same for their content types). Chat itself stays
      // content-type-agnostic.
      const { chatExtensionRegistry } = await import(
        '@/modules/chat/core/extensions'
      )
      await chatExtensionRegistry.onMessageEditRestore(
        precedingUserMsg.contents,
      )

      // Pre-fill text input with the original user message text. Skip only
      // the pre-fill when the preceding user message is attachment-only (no
      // text) — the regeneration itself must still proceed below.
      if (userText) (get() as any).TextStore?.setText(userText)

      // Mark as assistant-level fork so computeForkPoints anchors the
      // navigator at the assistant bubble on both parent and child branches
      set({
        pendingBranchForkLevel: 'assistant',
        pendingBranchFromMessageId: precedingUserMsg.id,
      })

      // Trim the user message and everything after so the UI shows a clean
      // state during streaming
      await get().trimMessagesToForkPoint(precedingUserMsg.id)

      await get().sendMessage()
    }
}
