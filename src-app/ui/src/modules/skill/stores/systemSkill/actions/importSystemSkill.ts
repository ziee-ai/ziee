import { ApiClient } from '@/api-client'
import type { ApiEndpointParameters, Skill } from '@/api-client/types'
import type { SystemSkillGet, SystemSkillSet } from '../state'

export default (set: SystemSkillSet, _get: SystemSkillGet) =>
  async (form: FormData): Promise<Skill> => {
    set(draft => {
      draft.creating = true
      draft.error = null
    })
    try {
      const skill = await ApiClient.Skill.import(
        form as ApiEndpointParameters['Skill.import'],
      )
      set(draft => {
        const idx = draft.systemSkills.findIndex(s => s.id === skill.id)
        if (idx >= 0) draft.systemSkills[idx] = skill
        else draft.systemSkills.push(skill)
        draft.creating = false
      })
      return skill
    } catch (error) {
      set(draft => {
        draft.creating = false
        draft.error =
          error instanceof Error ? error.message : 'Failed to import system skill'
      })
      throw error
    }
  }
