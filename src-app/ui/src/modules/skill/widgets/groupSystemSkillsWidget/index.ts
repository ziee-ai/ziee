import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { groupSystemSkillsWidgetState, type GroupSystemSkillsWidgetState } from './state'
import type { Actions } from './actions.gen'

const GroupSystemSkillsWidgetDef = defineStore<GroupSystemSkillsWidgetState, Actions>(
  'GroupSystemSkillsWidget',
  {
    immer: true,
    state: groupSystemSkillsWidgetState,
    actions: import.meta.glob('./actions/*.ts'),
  },
)

export const GroupSystemSkillsWidget = registerLazyStore(GroupSystemSkillsWidgetDef)
export const useGroupSystemSkillsWidgetStore = GroupSystemSkillsWidgetDef.store
