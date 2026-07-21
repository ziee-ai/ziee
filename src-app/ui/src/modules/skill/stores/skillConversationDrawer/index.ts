import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { skillConversationDrawerState } from './state'
import type { Actions } from './actions.gen'
import type { SkillConversationDrawerState } from './state'

const SkillConversationDrawerDef = defineStore<
  SkillConversationDrawerState,
  Actions
>('SkillConversationDrawer', {
  immer: true,
  state: skillConversationDrawerState,
  actions: import.meta.glob('./actions/*.ts'),
})

export const SkillConversationDrawer = registerLazyStore(SkillConversationDrawerDef)
export const useSkillConversationDrawerStore =
  SkillConversationDrawerDef.store
