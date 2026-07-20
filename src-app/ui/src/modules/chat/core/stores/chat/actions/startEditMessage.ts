import type { ChatSet, ChatInitialState, ChatState } from '@/modules/chat/core/stores/Chat.store'

export default (set: ChatSet, getRaw: () => ChatInitialState) => {
  const get = getRaw as unknown as () => ChatState
  return async (messageId: string) => {
      const message = get().messages.get(messageId)
      if (!message || message.role !== 'user') return

      // Trim messages to fork point so UI shows clean branch base immediately
      get().trimMessagesToForkPoint(messageId)

      // Set editing state — extensions subscribe to editingMessage via
      // useChatStore.subscribe() in their initialize() hooks
      set({
        editingMessage: message,
        pendingBranchFromMessageId: messageId,
        pendingBranchForkLevel: 'user',
      })

      // Pre-fill text input with message text content
      const textContent = message.contents
        .filter(c => c.content_type === 'text')
        .map(c => (c.content as any).text as string)
        .join('')
      ;(get() as any).TextStore?.setText(textContent)
    }
}
