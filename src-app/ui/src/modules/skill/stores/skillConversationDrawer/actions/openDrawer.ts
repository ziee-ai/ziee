import type { SkillConversationDrawerSet } from '../state'

export default (set: SkillConversationDrawerSet) =>
  async (conversationId: string) => {
    set(d => {
      d.openConversationId = conversationId
    })
  }
