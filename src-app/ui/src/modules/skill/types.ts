import type { StoreProxy } from '@/core/stores'
import type {
  useConversationSkillsStore,
  useSkillConversationDrawerStore,
  useSkillDrawerStore,
  useSkillStore,
  useSystemSkillStore,
} from '@/modules/skill/stores'

declare module '@/core/stores' {
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
  }
}

export {}
