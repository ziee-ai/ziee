// Project MCP-settings store — GET/PUT against /api/projects/{id}/mcp-settings.
// Mirrors the ProjectFiles pattern; the ProjectDetail subscription + its
// cleanup are now handled by store-kit's `watch` (no manual __unsubProjectDetail).

import { ApiClient } from '@/api-client'
import type {
  ProjectMcpSettingsRequest,
  ProjectMcpSettingsResponse,
} from '@/api-client/types'
import { Stores } from '@/core/stores'
import { useProjectDetailStore } from '@/modules/projects/stores'
import { defineStore } from '@/core/store-kit'

/** Canonical in-store representation (the GET-shape response). */
export type ProjectMcpSettings = ProjectMcpSettingsResponse

export const ProjectMcpSettingsStore = defineStore('ProjectMcpSettings', {
  immer: true,
  state: {
    currentProjectId: null as string | null,
    settings: null as ProjectMcpSettings | null,
    loading: false,
    saving: false,
    error: null as string | null,
  },
  actions: set => ({
    loadSettings: async (projectId: string) => {
      try {
        set({ loading: true, error: null })
        const settings = await ApiClient.Project.getMcpSettings({ id: projectId })
        set({ settings, loading: false })
      } catch (error) {
        set({
          error: error instanceof Error ? error.message : 'Failed to load MCP settings',
          loading: false,
        })
      }
    },
    updateSettings: async (
      projectId: string,
      payload: ProjectMcpSettingsRequest,
    ): Promise<ProjectMcpSettings> => {
      try {
        set({ saving: true, error: null })
        const updated = await ApiClient.Project.updateMcpSettings({ id: projectId, ...payload })
        set({ settings: updated, saving: false })
        await Stores.EventBus.emit({ type: 'project.mcp_updated', data: { projectId } })
        return updated
      } catch (error) {
        set({
          error: error instanceof Error ? error.message : 'Failed to update MCP settings',
          saving: false,
        })
        throw error
      }
    },
    clearError: () => set({ error: null }),
  }),
  init: ({ on, watch, set, get, actions }) => {
    // Mirror ProjectDetail's active project; reload on change. `watch`
    // auto-unsubscribes on destroy (was manual __unsubProjectDetail).
    watch(
      useProjectDetailStore,
      state => state.project?.id ?? null,
      newProjectId => {
        set(state => {
          state.currentProjectId = newProjectId
          state.settings = null
        })
        if (newProjectId) void actions.loadSettings(newProjectId)
      },
      { fireImmediately: true },
    )
    on('project.mcp_updated', async event => {
      const current = get().currentProjectId
      if (current && current === event.data.projectId) await actions.loadSettings(current)
    })
    on('project.deleted', async event => {
      const current = get().currentProjectId
      if (current && current === event.data.projectId) {
        set(state => {
          state.currentProjectId = null
          state.settings = null
        })
      }
    })
  },
})

export const useProjectMcpSettingsStore = ProjectMcpSettingsStore.store
