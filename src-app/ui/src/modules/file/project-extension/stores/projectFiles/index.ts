import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { projectFilesState, type ProjectFilesState } from './state'
import type { Actions } from './actions.gen'
import { useProjectDetailStore } from '@/modules/projects/stores'

const ProjectFilesDef = defineStore<ProjectFilesState, Actions>('ProjectFiles', {
  immer: true,
  state: projectFilesState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, watch, set, get, actions }) => {
    // Mirror the active project's id and reload files when it changes — atomic
    // reset on project change. Replaces the raw `useProjectDetailStore.subscribe`.
    watch(
      useProjectDetailStore,
      state => state.project?.id ?? null,
      newProjectId => {
        set(state => {
          state.currentProjectId = newProjectId
          state.files = []
          state.uploadingFiles.clear()
          state.selectedFileIds.clear()
        })
        if (newProjectId) void actions.loadFiles(newProjectId)
      },
      { fireImmediately: true },
    )

    // Refresh on attach (e.g. from another tab / sibling emit).
    on('project.file_attached', async event => {
      const current = get().currentProjectId
      if (current && current === event.data.projectId) {
        await actions.loadFiles(current)
      }
    })

    // Local removal on detach — avoids a full reload for a single-row drop.
    on('project.file_detached', async event => {
      const current = get().currentProjectId
      if (current && current === event.data.projectId) {
        set(state => {
          state.files = state.files.filter(f => f.id !== event.data.fileId)
          state.selectedFileIds.delete(event.data.fileId)
        })
      }
    })

    // Clear all state when the project is deleted.
    on('project.deleted', async event => {
      const current = get().currentProjectId
      if (current && current === event.data.projectId) {
        set(state => {
          state.currentProjectId = null
          state.files = []
          state.uploadingFiles.clear()
          state.selectedFileIds.clear()
        })
      }
    })
  },
})

// The registered lazy-store proxy (default export, used by module.tsx).
export const ProjectFiles = registerLazyStore(ProjectFilesDef)

// Re-export the raw defineStore handle so gallery seed code can reach
// `.store.getState()` / `.store.setState()` — same pattern other galleries
// use for non-migrated stores.
export { ProjectFilesDef }
export const useProjectFilesStore = ProjectFilesDef.store
