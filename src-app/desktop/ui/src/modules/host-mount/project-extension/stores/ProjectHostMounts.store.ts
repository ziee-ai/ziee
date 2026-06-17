// Project host-mounts store (desktop bundle).
//
// GET + PUT round-trips against `/api/host-mounts/project/{project_id}`.
// Subscribes to the active project (Stores.ProjectDetail) and reloads on
// change. Mirrors `mcp/project-extension` (core) + `RemoteAccess.store` (desktop).

import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'

import { ApiClient } from '@/api-client'
import type { MountEntry } from '@/api-client/types'
import { type StoreProxy } from '@/core/stores'
// Raw zustand hook for the subscription — going through Stores.ProjectDetail
// would fire the proxy's internal hooks (corrupts hook count). Same lesson as
// the core mcp/project-extension store.
import { useProjectDetailStore } from '@ziee/ui-core/modules/projects/stores'

interface ProjectHostMountsState {
  currentProjectId: string | null
  mounts: MountEntry[]
  loading: boolean
  saving: boolean
  error: string | null

  __init__: {
    __store__: () => void
  }

  loadMounts: (projectId: string) => Promise<void>
  saveMounts: (projectId: string, mounts: MountEntry[]) => Promise<void>
  clearError: () => void
}

declare module '@/core/stores' {
  interface RegisteredStores {
    ProjectHostMounts: StoreProxy<ProjectHostMountsState>
  }
}

export const useProjectHostMountsStore = create<ProjectHostMountsState>()(
  subscribeWithSelector(
    immer(
      (set, get): ProjectHostMountsState => ({
        currentProjectId: null,
        mounts: [],
        loading: false,
        saving: false,
        error: null,

        __init__: {
          __store__: () => {
            // Mirror ProjectDetail's active project; reload on change.
            useProjectDetailStore.subscribe(
              (state) => state.project?.id ?? null,
              (newProjectId) => {
                set((state) => {
                  state.currentProjectId = newProjectId
                  state.mounts = []
                })
                if (newProjectId) {
                  void get().loadMounts(newProjectId)
                }
              },
              { fireImmediately: true },
            )
          },
        },

        loadMounts: async (projectId) => {
          try {
            set({ loading: true, error: null })
            const body = await ApiClient.HostMount.getProjectMounts({
              project_id: projectId,
            })
            set({ mounts: body.mounts, loading: false })
          } catch (error) {
            set({
              error:
                error instanceof Error ? error.message : 'Failed to load mounts',
              loading: false,
            })
          }
        },

        saveMounts: async (projectId, mounts) => {
          try {
            set({ saving: true, error: null })
            const body = await ApiClient.HostMount.putProjectMounts({
              project_id: projectId,
              mounts,
            })
            set({ mounts: body.mounts, saving: false })
          } catch (error) {
            set({
              error:
                error instanceof Error ? error.message : 'Failed to save mounts',
              saving: false,
            })
            throw error
          }
        },

        clearError: () => set({ error: null }),
      }),
    ),
  ),
)
