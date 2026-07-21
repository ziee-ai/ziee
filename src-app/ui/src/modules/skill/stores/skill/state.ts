import type { StoreSet } from '@ziee/framework/store-kit'
import type { Skill } from '@/api-client/types'

export const skillState = {
  skills: [] as Skill[],
  isInitialized: false,
  loading: false,
  creating: false,
  error: null as string | null,
  operationsLoading: {} as Record<string, boolean>,
}

export type SkillState = typeof skillState
export type SkillSet = StoreSet<SkillState>
export type SkillGet = () => SkillState
