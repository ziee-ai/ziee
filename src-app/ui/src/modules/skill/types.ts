import type { StoreProxy } from '@/core/stores'
import type {
  useConversationSkillsStore,
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
  }
}

export {}
