import type { StoreProxy } from '@ziee/framework/stores'
import type {
  useProjectsStore,
  useProjectDetailStore,
  useProjectDrawerStore,
} from '@/modules/projects/stores'

declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    Projects: StoreProxy<ReturnType<typeof useProjectsStore.getState>>
    ProjectDetail: StoreProxy<ReturnType<typeof useProjectDetailStore.getState>>
    ProjectDrawer: StoreProxy<ReturnType<typeof useProjectDrawerStore.getState>>
  }
}

export {}
