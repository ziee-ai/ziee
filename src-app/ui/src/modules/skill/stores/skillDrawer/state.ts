import type { Skill } from '@/api-client/types'
import type { StoreSet } from '@ziee/framework/store-kit'

export const skillDrawerState = {
  isOpen: false,
  skill: null as Skill | null,
  // Optional conversation context — when set, the drawer surfaces the
  // "Hide in this conversation" checkbox.
  conversationId: null as string | null,
}

export type SkillDrawerState = typeof skillDrawerState
export type SkillDrawerSet = StoreSet<SkillDrawerState>
export type SkillDrawerGet = () => SkillDrawerState
