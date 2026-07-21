import { ApiClient } from '@/api-client'
import type { ApiEndpointParameters, Skill } from '@/api-client/types'
import type { SkillGet, SkillSet } from '../state'

export default (set: SkillSet, _get: SkillGet) =>
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
        const idx = draft.skills.findIndex(s => s.id === skill.id)
        if (idx >= 0) draft.skills[idx] = skill
        else draft.skills.push(skill)
        draft.creating = false
      })
      return skill
    } catch (error) {
      set(draft => {
        draft.creating = false
        draft.error = error instanceof Error ? error.message : 'Failed to import skill'
      })
      throw error
    }
  }
