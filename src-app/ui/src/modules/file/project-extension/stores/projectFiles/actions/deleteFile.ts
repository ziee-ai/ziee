import { ApiClient } from '@/api-client'
import { emitProjectFileDetached } from '@/modules/file/project-extension/events'
import type { ProjectFilesGet, ProjectFilesSet } from '../state'

export default (set: ProjectFilesSet, _get: ProjectFilesGet) =>
  async (projectId: string, fileId: string) => {
    try {
      set({ detaching: true, error: null })
      await ApiClient.File.delete({ file_id: fileId })
      await emitProjectFileDetached(projectId, fileId)
      set({ detaching: false })
    } catch (error) {
      set({
        error:
          error instanceof Error ? error.message : 'Failed to delete file',
        detaching: false,
      })
      throw error
    }
  }
