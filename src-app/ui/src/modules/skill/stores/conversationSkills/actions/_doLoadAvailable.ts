import { ApiClient } from '@/api-client'
import type { ConversationSkillsGet, ConversationSkillsSet } from '../state'

export default (set: ConversationSkillsSet, _get: ConversationSkillsGet) =>
  async (conversationId: string) => {
    set(draft => {
      draft.loading[conversationId] = true
      draft.error = null
    })
    try {
      const response = await ApiClient.Skill.listAvailable({
        conversation_id: conversationId,
      })
      set(draft => {
        draft.available[conversationId] = response.skills
        draft.loading[conversationId] = false
      })
    } catch (error) {
      set(draft => {
        draft.loading[conversationId] = false
        draft.error =
          error instanceof Error ? error.message : 'Failed to load available skills'
      })
    }
  }
