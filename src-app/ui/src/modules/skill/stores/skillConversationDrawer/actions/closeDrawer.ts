import type { SkillConversationDrawerSet } from '../state'

export default (set: SkillConversationDrawerSet) =>
  async () => {
    set(d => {
      d.openConversationId = null
    })
  }
