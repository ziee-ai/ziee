import { ApiClient } from '@/api-client'
import type { SystemSkillGet, SystemSkillSet } from '../state'

export default (set: SystemSkillSet, _get: SystemSkillGet) => async (skillId: string) => {
  set(draft => {
    draft.groups[skillId] = {
      groupIds: draft.groups[skillId]?.groupIds ?? [],
      loading: true,
    }
  })
  try {
    const groupIds = await ApiClient.SkillSystem.getGroups({ id: skillId })
    set(draft => {
      draft.groups[skillId] = { groupIds, loading: false }
    })
  } catch (error) {
    set(draft => {
      draft.groups[skillId] = {
        groupIds: draft.groups[skillId]?.groupIds ?? [],
        loading: false,
      }
      draft.error =
        error instanceof Error ? error.message : 'Failed to load skill groups'
    })
  }
}
