import type { GroupSystemSkillsAssignmentDrawerGet, GroupSystemSkillsAssignmentDrawerSet } from '../state'

export default (set: GroupSystemSkillsAssignmentDrawerSet, _get: GroupSystemSkillsAssignmentDrawerGet) =>
  async () => {
    set({ isOpen: false, selectedGroup: null })
  }
