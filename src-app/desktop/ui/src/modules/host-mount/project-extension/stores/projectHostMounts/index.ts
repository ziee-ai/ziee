import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { projectHostMountsState, type ProjectHostMountsState } from './state'
import type { Actions } from './actions.gen'

// Re-export the state type so existing consumers that import it are satisfied.
export type { ProjectHostMountsState } from './state'

// Raw zustand hook for the watch — going through ProjectDetailStore would fire
// the proxy's internal hooks (corrupts hook count).
import { useProjectDetailStore } from '@ziee/ui-core/modules/projects/stores'

const ProjectHostMountsDef = defineStore<ProjectHostMountsState, Actions>('ProjectHostMounts', {
  immer: true,
  state: projectHostMountsState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ actions, set, watch }) => {
    // Mirror ProjectDetail's active project; reload on change.
    watch(
      useProjectDetailStore,
      state => state.project?.id ?? null,
      newProjectId => {
        set(s => {
          s.currentProjectId = newProjectId
          s.mounts = []
        })
        if (newProjectId) void actions.loadMounts(newProjectId)
      },
      { fireImmediately: true },
    )
  },
})
export const ProjectHostMounts = registerLazyStore(ProjectHostMountsDef)
export const useProjectHostMountsStore = ProjectHostMountsDef.store

// Keep the legacy module-augmentation declaration so the Stores proxy is typed.
declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    ProjectHostMounts: import('@ziee/framework/stores').StoreProxy<
      ReturnType<typeof useProjectHostMountsStore.getState>
    >
  }
}
