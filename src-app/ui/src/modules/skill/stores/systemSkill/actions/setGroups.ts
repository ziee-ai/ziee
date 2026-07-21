import { ApiClient } from '@/api-client'
import type { SystemSkillGet, SystemSkillSet } from '../state'

export default (set: SystemSkillSet, _get: SystemSkillGet) =>
  async (skillId: string, groupIds: string[]) => {
    await ApiClient.SkillSystem.setGroups({ id: skillId, group_ids: groupIds })
    set(draft => {
      draft.groups[skillId] = { groupIds, loading: false }
    })
  }
