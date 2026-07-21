import { ApiClient } from '@/api-client'
import type { MountEntry } from '@/api-client/types'
import type { ConversationHostMountsSet } from '../state'

export default (set: ConversationHostMountsSet) =>
  async (conversationId: string, mounts: MountEntry[]) => {
    set(s => {
      s.saving = true
      s.error = null
    })
    try {
      const body = await ApiClient.HostMount.putConversationMounts({
        conversation_id: conversationId,
        mounts,
      })
      set(s => {
        s.byConversation[conversationId] = body.mounts
        s.saving = false
      })
    } catch (error) {
      set(s => {
        s.error =
          error instanceof Error
            ? error.message
            : 'Failed to save mounts'
        s.saving = false
      })
      throw error
    }
  }
