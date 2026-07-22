import { setOverlayOpen } from '@/core/overlays/overlayVisibility'
import type { GroupSystemSkillsAssignmentDrawerGet, GroupSystemSkillsAssignmentDrawerSet } from '../state'

export default (set: GroupSystemSkillsAssignmentDrawerSet, _get: GroupSystemSkillsAssignmentDrawerGet) =>
  async () => {
    set({ isOpen: false, selectedGroup: null })
    setOverlayOpen('group-skill-assignment', false)
  }
