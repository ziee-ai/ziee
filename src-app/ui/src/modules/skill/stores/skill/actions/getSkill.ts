import { ApiClient } from '@/api-client'
import type { Skill } from '@/api-client/types'
import type { SkillGet, SkillSet } from '../state'

export default (set: SkillSet, _get: SkillGet) =>
  async (id: string): Promise<Skill> => {
    const skill = await ApiClient.Skill.get({ id })
    set(draft => {
      const idx = draft.skills.findIndex(s => s.id === id)
      if (idx >= 0) draft.skills[idx] = skill
    })
    return skill
  }
