// Project MCP-settings store.
//
// Owns the GET + PUT round-trips against `/api/projects/{id}/mcp-settings`.
// The `Project` payload no longer carries `mcp_*` fields after the
// unification (migration 78), so the panel fetches them here.
//
// Single-project scope: subscribes to `Stores.ProjectDetail.project.id`
// and reloads on change. Mirrors the file/project-extension pattern.

import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import type {
  ProjectMcpSettingsRequest,
  ProjectMcpSettingsResponse,
} from '@/api-client/types'
import { Stores } from '@/core/stores'
// Raw zustand hook for the subscription — going through Stores.ProjectDetail
// would fire the proxy's internal useEffect+useStore hooks (corrupts hook
// count). Same lesson as the file/project-extension ProjectFiles store.
import { useProjectDetailStore } from '@/modules/projects/stores'

/**
 * Re-exported autogen view shape so consumers can `import { ProjectMcpSettings }
 * from '...'` without reaching into the api-client. The GET-shape response
 * is the canonical in-store representation.
 */
export type ProjectMcpSettings = ProjectMcpSettingsResponse

interface ProjectMcpSettingsState {
  /** Currently-active project id, mirrored from ProjectDetail. */
  currentProjectId: string | null
  settings: ProjectMcpSettings | null
  loading: boolean
  saving: boolean
  error: string | null

  /** Unsubscribe from the ProjectDetail store subscription (cleaned up
   *  in __destroy__ so subscriptions don't accumulate on re-init). */
  __unsubProjectDetail?: () => void

  __init__: {
    __store__: () => void
  }
  __destroy__?: () => void

  loadSettings: (projectId: string) => Promise<void>
  updateSettings: (
    projectId: string,
    payload: ProjectMcpSettingsRequest,
  ) => Promise<ProjectMcpSettings>
  clearError: () => void
}

export const useProjectMcpSettingsStore = create<ProjectMcpSettingsState>()(
  subscribeWithSelector(
    immer(
      (set, get): ProjectMcpSettingsState => ({
        currentProjectId: null,
        settings: null,
        loading: false,
        saving: false,
        error: null,

        __init__: {
          __store__: () => {
            const GROUP = 'ProjectMcpSettingsStore'
            const eventBus = Stores.EventBus

            // Mirror ProjectDetail's active project. Reload on change.
            const unsubProjectDetail = useProjectDetailStore.subscribe(
              state => state.project?.id ?? null,
              newProjectId => {
                set(state => {
                  state.currentProjectId = newProjectId
                  state.settings = null
                })
                if (newProjectId) {
                  void get().loadSettings(newProjectId)
                }
              },
              { fireImmediately: true },
            )

            // Persist the unsubscribe so __destroy__ can clean it up.
            set(state => {
              state.__unsubProjectDetail = unsubProjectDetail
            })

            // Other subscribers can trigger a refresh by emitting
            // project.mcp_updated (e.g. cross-tab sync).
            eventBus.on(
              'project.mcp_updated',
              async event => {
                const current = get().currentProjectId
                if (current && current === event.data.projectId) {
                  await get().loadSettings(current)
                }
              },
              GROUP,
            )

            eventBus.on(
              'project.deleted',
              async event => {
                const current = get().currentProjectId
                if (current && current === event.data.projectId) {
                  set(state => {
                    state.currentProjectId = null
                    state.settings = null
                  })
                }
              },
              GROUP,
            )
          },
        },

        loadSettings: async projectId => {
          try {
            set({ loading: true, error: null })
            const settings = await ApiClient.Project.getMcpSettings({
              id: projectId,
            })
            set({ settings, loading: false })
          } catch (error) {
            set({
              error:
                error instanceof Error
                  ? error.message
                  : 'Failed to load MCP settings',
              loading: false,
            })
          }
        },

        updateSettings: async (projectId, payload) => {
          try {
            set({ saving: true, error: null })
            const updated = await ApiClient.Project.updateMcpSettings({
              id: projectId,
              ...payload,
            })
            set({ settings: updated, saving: false })
            await Stores.EventBus.emit({
              type: 'project.mcp_updated',
              data: { projectId },
            })
            return updated
          } catch (error) {
            set({
              error:
                error instanceof Error
                  ? error.message
                  : 'Failed to update MCP settings',
              saving: false,
            })
            throw error
          }
        },

        clearError: () => set({ error: null }),

        __destroy__: () => {
          // Tear down the cross-module zustand subscription so it doesn't
          // leak if the store is re-initialized.
          get().__unsubProjectDetail?.()
          Stores.EventBus.removeGroupListeners('ProjectMcpSettingsStore')
        },
      }),
    ),
  ),
)
