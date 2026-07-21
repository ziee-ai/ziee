import { ApiClient } from '@/api-client'
import type { ConversationSkillsGet, ConversationSkillsSet } from '../state'
import loadAvailableFactory from './loadAvailable'

export default (set: ConversationSkillsSet, get: ConversationSkillsGet) => {
  const loadAvailable = loadAvailableFactory(set, get)
  return async (skillId: string, conversationId: string) => {
    try {
      await ApiClient.Skill.unhideInConversation({
        id: skillId,
        conversation_id: conversationId,
      })
      await loadAvailable(conversationId)
    } catch (error) {
      set(draft => {
        draft.error =
          error instanceof Error ? error.message : 'Failed to unhide skill'
      })
      throw error
    }
  }
}
