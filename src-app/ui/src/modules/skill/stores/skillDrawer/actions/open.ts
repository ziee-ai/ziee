import type { Skill } from '@/api-client/types'
import type { SkillDrawerSet } from '../state'

export default (set: SkillDrawerSet) =>
  async (skill: Skill, conversationId?: string) => {
    set(d => {
      d.isOpen = true
      d.skill = skill
      d.conversationId = conversationId ?? null
    })
  }
