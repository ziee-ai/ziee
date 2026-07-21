import { ApiClient } from '@/api-client'
import type { SystemSkillGet, SystemSkillSet } from '../state'

export default (set: SystemSkillSet, _get: SystemSkillGet) => async (id: string) => {
  await ApiClient.SkillSystem.delete({ id })
  set(draft => {
    draft.systemSkills = draft.systemSkills.filter(s => s.id !== id)
  })
}
