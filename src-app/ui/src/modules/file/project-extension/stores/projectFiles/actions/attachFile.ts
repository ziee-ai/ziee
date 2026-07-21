import { ApiClient } from '@/api-client'
import { emitProjectFileAttached } from '@/modules/file/project-extension/events'
import type { ProjectFilesGet, ProjectFilesSet } from '../state'

export default (set: ProjectFilesSet, _get: ProjectFilesGet) =>
  async (projectId: string, fileId: string) => {
    try {
      set({ attaching: true, error: null })
      await ApiClient.Project.attachFile({ id: projectId, file_id: fileId })
      await emitProjectFileAttached(projectId, fileId)
      set({ attaching: false })
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to attach file',
        attaching: false,
      })
      throw error
    }
  }
