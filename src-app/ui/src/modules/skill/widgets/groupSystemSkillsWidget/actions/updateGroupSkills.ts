import { ApiClient } from '@/api-client'
import type { GroupSystemSkillsWidgetGet, GroupSystemSkillsWidgetSet } from '../state'

export default (set: GroupSystemSkillsWidgetSet, _get: GroupSystemSkillsWidgetGet) =>
  async (groupId: string, skillIds: string[]) => {
    const response = await ApiClient.Group.updateSystemSkills({
      group_id: groupId,
      skill_ids: skillIds,
    })
    set(state => {
      state.groupSkills.set(groupId, {
        groupId,
        skills: response.skills,
        loading: false,
        error: null,
        lastFetched: Date.now(),
      })
    })
  }
