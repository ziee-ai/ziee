import { ApiClient } from '@/api-client'
import type { UpdateSkill } from '@/api-client/types'
import type { SystemSkillGet, SystemSkillSet } from '../state'

export default (set: SystemSkillSet, _get: SystemSkillGet) =>
  async (id: string, data: UpdateSkill) => {
    const updated = await ApiClient.SkillSystem.update({ id, ...data })
    set(draft => {
      const idx = draft.systemSkills.findIndex(s => s.id === id)
      if (idx >= 0) draft.systemSkills[idx] = updated
    })
    return updated
  }
