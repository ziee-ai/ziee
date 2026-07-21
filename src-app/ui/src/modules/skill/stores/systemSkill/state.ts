import type { StoreSet } from '@ziee/framework/store-kit'
import type { Skill } from '@/api-client/types'

export const systemSkillState = {
  systemSkills: [] as Skill[],
  isInitialized: false,
  loading: false,
  creating: false,
  error: null as string | null,
  // Per-skill assigned group ids (lazy-loaded by the assignment card).
  groups: {} as Record<string, { groupIds: string[]; loading: boolean }>,
}

export type SystemSkillState = typeof systemSkillState
export type SystemSkillSet = StoreSet<SystemSkillState>
export type SystemSkillGet = () => SystemSkillState
