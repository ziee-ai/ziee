import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import {
  projectMcpSettingsState,
  type ProjectMcpSettingsState,
} from './state'
import type { Actions } from './actions.gen'
import { useProjectDetailStore } from '@/modules/projects/stores'

// Re-export the canonical type (ProjectMcpSettingsResponse shape) so
// inline import() type-resolution in McpComposer.store.ts resolves.
export type {
  ProjectMcpSettings as ProjectMcpSettings,
} from './state'

const ProjectMcpSettingsStore = defineStore<ProjectMcpSettingsState, Actions>(
  'ProjectMcpSettings',
  {
    immer: true,
    state: projectMcpSettingsState,
    actions: import.meta.glob('./actions/*.ts'),
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
        if (current && current === event.data.projectId)
          await actions.loadSettings(current)
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
  },
)
// The raw defineStore handle so gallery seed code can reach `.store.setState()`
// — same pattern projectFiles/index.ts uses for ProjectFilesDef.
export { ProjectMcpSettingsStore }
export const ProjectMcpSettingsLazy = registerLazyStore(ProjectMcpSettingsStore)
export const useProjectMcpSettingsStore = ProjectMcpSettingsStore.store
