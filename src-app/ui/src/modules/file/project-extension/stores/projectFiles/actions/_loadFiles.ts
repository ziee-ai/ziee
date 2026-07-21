import { ApiClient } from '@/api-client'
import type { ProjectFilesGet, ProjectFilesSet } from '../state'

/**
 * Internal factory — loads project files from the API. Exported as a factory
 * so sibling actions that need to call it get a typed closure instead of
 * reaching through `get()` (which is state-only by design).
 */
export default (set: ProjectFilesSet, _get: ProjectFilesGet) =>
  async (projectId: string) => {
    try {
      set({ filesLoading: true })
      const response = await ApiClient.Project.listFiles({ id: projectId })
      set({ files: response.files, filesLoading: false })
    } catch (error) {
      set({
        error:
          error instanceof Error ? error.message : 'Failed to load project files',
        filesLoading: false,
      })
    }
  }
