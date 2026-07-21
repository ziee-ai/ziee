import type { StoreSet } from '@ziee/framework/store-kit'
import type { Group } from '@/api-client/types'

export const groupSystemSkillsAssignmentDrawerState = {
  isOpen: false,
  selectedGroup: null as Group | null,
}

export type GroupSystemSkillsAssignmentDrawerState = typeof groupSystemSkillsAssignmentDrawerState
export type GroupSystemSkillsAssignmentDrawerSet = StoreSet<GroupSystemSkillsAssignmentDrawerState>
export type GroupSystemSkillsAssignmentDrawerGet = () => GroupSystemSkillsAssignmentDrawerState
