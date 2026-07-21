import type { StoreSet } from '@ziee/framework/store-kit'

export const skillConversationDrawerState = {
  openConversationId: null as string | null,
}

export type SkillConversationDrawerState = typeof skillConversationDrawerState
export type SkillConversationDrawerSet = StoreSet<SkillConversationDrawerState>
export type SkillConversationDrawerGet = () => SkillConversationDrawerState
