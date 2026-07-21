import type { Group } from '@/api-client/types'
import type { GroupSystemSkillsAssignmentDrawerGet, GroupSystemSkillsAssignmentDrawerSet } from '../state'

export default (set: GroupSystemSkillsAssignmentDrawerSet, _get: GroupSystemSkillsAssignmentDrawerGet) =>
  async (group: Group) => {
    set({ isOpen: true, selectedGroup: group })
  }
