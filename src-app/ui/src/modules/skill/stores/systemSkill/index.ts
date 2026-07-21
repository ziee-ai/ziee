import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { systemSkillState, type SystemSkillState } from './state'
import type { Actions } from './actions.gen'

const SystemSkillDef = defineStore<SystemSkillState, Actions>('SystemSkill', {
  immer: true,
  state: systemSkillState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, actions }) => {
    const reload = () => void actions.loadSystemSkills()
    on('sync:skill_system', reload)
    on('sync:reconnect', reload)
    void actions.loadSystemSkills()
  },
})
export const SystemSkill = registerLazyStore(SystemSkillDef)
export const useSystemSkillStore = SystemSkillDef.store
