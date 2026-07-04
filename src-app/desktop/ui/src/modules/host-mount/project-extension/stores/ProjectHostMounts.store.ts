// Project host-mounts store (desktop bundle).
//
// GET + PUT round-trips against `/api/host-mounts/project/{project_id}`.
// Watches the active project (ProjectDetail) and reloads on change.

import { ApiClient } from '@/api-client'
import type { MountEntry } from '@/api-client/types'
import { defineStore } from '@/core/store-kit'
import { type StoreProxy } from '@/core/stores'
// Raw zustand hook for the watch — going through Stores.ProjectDetail would fire
// the proxy's internal hooks (corrupts hook count).
import { useProjectDetailStore } from '@ziee/ui-core/modules/projects/stores'

interface ProjectHostMountsState {
  currentProjectId: string | null
  mounts: MountEntry[]
  loading: boolean
  saving: boolean
  error: string | null
  loadMounts: (projectId: string) => Promise<void>
  saveMounts: (projectId: string, mounts: MountEntry[]) => Promise<void>
  clearError: () => void
}

declare module '@/core/stores' {
  interface RegisteredStores {
    ProjectHostMounts: StoreProxy<ProjectHostMountsState>
  }
}

export const ProjectHostMounts = defineStore('ProjectHostMounts', {
  immer: true,
  state: {
    currentProjectId: null as string | null,
    mounts: [] as MountEntry[],
    loading: false,
    saving: false,
    error: null as string | null,
  },
  actions: set => ({
    loadMounts: async (projectId: string) => {
      try {
        set({ loading: true, error: null })
        const body = await ApiClient.HostMount.getProjectMounts({ project_id: projectId })
        set({ mounts: body.mounts, loading: false })
      } catch (error) {
        set({
          error: error instanceof Error ? error.message : 'Failed to load mounts',
          loading: false,
        })
      }
    },
    saveMounts: async (projectId: string, mounts: MountEntry[]) => {
      try {
        set({ saving: true, error: null })
        const body = await ApiClient.HostMount.putProjectMounts({ project_id: projectId, mounts })
        set({ mounts: body.mounts, saving: false })
      } catch (error) {
        set({
          error: error instanceof Error ? error.message : 'Failed to save mounts',
          saving: false,
        })
        throw error
      }
    },
    clearError: () => set({ error: null }),
  }),
  init: ({ watch, set, actions }) => {
    // Mirror ProjectDetail's active project; reload on change.
    watch(
      useProjectDetailStore,
      state => state.project?.id ?? null,
      newProjectId => {
        set(state => {
          state.currentProjectId = newProjectId
          state.mounts = []
        })
        if (newProjectId) void actions.loadMounts(newProjectId)
      },
      { fireImmediately: true },
    )
  },
})

export const useProjectHostMountsStore = ProjectHostMounts.store
