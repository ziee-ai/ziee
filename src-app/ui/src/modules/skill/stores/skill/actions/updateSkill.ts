import { ApiClient } from '@/api-client'
import type { Skill, UpdateSkill } from '@/api-client/types'
import type { SkillGet, SkillSet } from '../state'

export default (set: SkillSet, _get: SkillGet) =>
  async (id: string, data: UpdateSkill): Promise<Skill> => {
    set(draft => {
      draft.operationsLoading[id] = true
      draft.error = null
    })
    try {
      const updated = await ApiClient.Skill.update({ id, ...data })
      set(draft => {
        const idx = draft.skills.findIndex(s => s.id === id)
        if (idx >= 0) draft.skills[idx] = updated
        delete draft.operationsLoading[id]
      })
      return updated
    } catch (error) {
      set(draft => {
        delete draft.operationsLoading[id]
        draft.error = error instanceof Error ? error.message : 'Failed to update skill'
      })
      throw error
    }
  }
