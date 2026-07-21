import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { skillDrawerState, type SkillDrawerState } from './state'
import type { Actions } from './actions.gen'

const SkillDrawerDef = defineStore<SkillDrawerState, Actions>('SkillDrawer', {
  immer: true,
  state: skillDrawerState,
  actions: import.meta.glob('./actions/*.ts'),
})

export const SkillDrawer = registerLazyStore(SkillDrawerDef)
export const useSkillDrawerStore = SkillDrawerDef.store
