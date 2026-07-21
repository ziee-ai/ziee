import type { Skill } from '@/api-client/types'
import type { StoreSet } from '@ziee/framework/store-kit'

export interface GroupSkills {
  groupId: string
  skills: Skill[]
  loading: boolean
  error: string | null
  lastFetched: number | null
}

export const groupSystemSkillsWidgetState = {
  groupSkills: new Map<string, GroupSkills>(),
}

export type GroupSystemSkillsWidgetState = typeof groupSystemSkillsWidgetState
export type GroupSystemSkillsWidgetSet = StoreSet<GroupSystemSkillsWidgetState>
export type GroupSystemSkillsWidgetGet = () => GroupSystemSkillsWidgetState
