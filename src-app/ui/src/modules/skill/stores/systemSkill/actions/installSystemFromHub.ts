import { ApiClient } from '@/api-client'
import type { Skill } from '@/api-client/types'
import type { SystemSkillGet, SystemSkillSet } from '../state'

export default (set: SystemSkillSet, _get: SystemSkillGet) =>
  async (hubId: string, groups?: string[]): Promise<Skill> => {
    set(draft => {
      draft.creating = true
      draft.error = null
    })
    try {
      const response = await ApiClient.Hub.createSystemSkillFromHub({
        hub_id: hubId,
        ...(groups && groups.length > 0 ? { groups } : {}),
      })
      set(draft => {
        draft.systemSkills.push(response.skill)
        draft.creating = false
      })
      return response.skill
    } catch (error) {
      set(draft => {
        draft.creating = false
        draft.error =
          error instanceof Error ? error.message : 'Failed to install system skill'
      })
      throw error
    }
  }
