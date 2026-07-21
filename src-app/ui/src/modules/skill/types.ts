import type { StoreProxy } from '@ziee/framework/stores'
import type {
  useConversationSkillsStore,
  useSkillConversationDrawerStore,
  useSkillDrawerStore,
  useSkillStore,
  useSystemSkillStore,
} from '@/modules/skill/stores'
import type { useGroupSystemSkillsWidgetStore } from '@/modules/skill/widgets/groupSystemSkillsWidget'
import type { useGroupSystemSkillsAssignmentStore } from '@/modules/skill/widgets/GroupSystemSkillsAssignmentDrawer.store'

declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    Skill: StoreProxy<ReturnType<typeof useSkillStore.getState>>
    ConversationSkills: StoreProxy<
      ReturnType<typeof useConversationSkillsStore.getState>
    >
    SystemSkill: StoreProxy<ReturnType<typeof useSystemSkillStore.getState>>
    SkillDrawer: StoreProxy<ReturnType<typeof useSkillDrawerStore.getState>>
    SkillConversationDrawer: StoreProxy<
      ReturnType<typeof useSkillConversationDrawerStore.getState>
    >
    GroupSystemSkillsWidget: StoreProxy<
      ReturnType<typeof useGroupSystemSkillsWidgetStore.getState>
    >
    GroupSystemSkillsAssignment: StoreProxy<
      ReturnType<typeof useGroupSystemSkillsAssignmentStore.getState>
    >
  }
}

export {}
