import { ApiClient } from '@/api-client'
import type { Skill } from '@/api-client/types'
import type { SkillGet, SkillSet } from '../state'

export default (set: SkillSet, _get: SkillGet) =>
  async (hubId: string): Promise<Skill> => {
    set(draft => {
      draft.creating = true
      draft.error = null
    })
    try {
      const response = await ApiClient.Hub.createSkillFromHub({ hub_id: hubId })
      set(draft => {
        draft.skills.push(response.skill)
        draft.creating = false
      })
      return response.skill
    } catch (error) {
      set(draft => {
        draft.creating = false
        draft.error = error instanceof Error ? error.message : 'Failed to install skill'
      })
      throw error
    }
  }
