import { ApiClient } from '@/api-client'
import type { ConversationHostMountsSet } from '../state'

export default (set: ConversationHostMountsSet) =>
  async (conversationId: string) => {
    set(s => {
      s.loading = true
      s.error = null
    })
    try {
      const body = await ApiClient.HostMount.getConversationMounts({
        conversation_id: conversationId,
      })
      set(s => {
        s.byConversation[conversationId] = body.mounts
        s.loading = false
      })
    } catch (error) {
      set(s => {
        s.error =
          error instanceof Error
            ? error.message
            : 'Failed to load mounts'
        s.loading = false
      })
    }
  }
