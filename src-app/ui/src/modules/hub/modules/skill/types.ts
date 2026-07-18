import type { StoreProxy } from '@ziee/framework/stores'
import type { useHubSkillsStore } from '@/modules/hub/modules/skill/stores/hub-skills-store'

declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    HubSkills: StoreProxy<ReturnType<typeof useHubSkillsStore.getState>>
  }
}

export {}
