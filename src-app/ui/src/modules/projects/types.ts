import type { StoreProxy } from '@/core/stores'
import type {
  useProjectsStore,
  useProjectDetailStore,
  useProjectDrawerStore,
} from '@/modules/projects/stores'

declare module '@/core/stores' {
  interface RegisteredStores {
    Projects: StoreProxy<ReturnType<typeof useProjectsStore.getState>>
    ProjectDetail: StoreProxy<ReturnType<typeof useProjectDetailStore.getState>>
    ProjectDrawer: StoreProxy<ReturnType<typeof useProjectDrawerStore.getState>>
  }
}

export {}
