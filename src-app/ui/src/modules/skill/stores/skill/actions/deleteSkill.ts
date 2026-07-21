import { ApiClient } from '@/api-client'
import type { SkillGet, SkillSet } from '../state'

export default (set: SkillSet, _get: SkillGet) =>
  async (id: string): Promise<void> => {
    set(draft => {
      draft.operationsLoading[id] = true
      draft.error = null
    })
    try {
      await ApiClient.Skill.delete({ id })
      set(draft => {
        draft.skills = draft.skills.filter(s => s.id !== id)
        delete draft.operationsLoading[id]
      })
    } catch (error) {
      set(draft => {
        delete draft.operationsLoading[id]
        draft.error = error instanceof Error ? error.message : 'Failed to delete skill'
      })
      throw error
    }
  }
