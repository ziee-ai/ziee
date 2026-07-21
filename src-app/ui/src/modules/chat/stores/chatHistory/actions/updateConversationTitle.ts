import { ApiClient } from '@/api-client'
import type { ChatHistoryGet, ChatHistorySet } from '../state'

export default (set: ChatHistorySet, _get: ChatHistoryGet) =>
  async (id: string, title: string) => {
    try {
      await ApiClient.Conversation.update({ id, title })
      set(draft => {
        const updateTitle = (conv: typeof draft.conversations[number]) => {
          if (conv.id === id) conv.title = title
        }
        draft.conversations.forEach(updateTitle)
        draft.recentConversations.forEach(updateTitle)
      })
    } catch (error) {
      console.error('[ChatHistory] Failed to update conversation title:', error)
      set({ error: 'Failed to update conversation title' })
      throw error
    }
  }
