import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { skillState, type SkillState } from './state'
import type { Actions } from './actions.gen'

const SkillDef = defineStore<SkillState, Actions>('Skill', {
  immer: true,
  state: skillState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, actions }) => {
    // Cross-device + local sync: the REST refetch is permission-gated internally
    // so a reconnect from a user without skills::read is a no-op.
    const reload = () => void actions.loadSkills()
    on('sync:skill', reload)
    on('sync:reconnect', reload)
    void actions.loadSkills()
  },
})
export const Skill = registerLazyStore(SkillDef)
export const useSkillStore = SkillDef.store
