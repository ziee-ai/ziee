import type { StoreProxy } from '@/core/stores'
import type { useHubSkillsStore } from '@/modules/hub/modules/skill/stores/hub-skills-store'

declare module '@/core/stores' {
  interface RegisteredStores {
    HubSkills: StoreProxy<ReturnType<typeof useHubSkillsStore.getState>>
  }
}

export {}
