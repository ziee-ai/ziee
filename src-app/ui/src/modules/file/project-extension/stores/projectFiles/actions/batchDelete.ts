import { ApiClient } from '@/api-client'
import { emitProjectFileDetached } from '@/modules/file/project-extension/events'
import type { ProjectFilesGet, ProjectFilesSet } from '../state'

export default (set: ProjectFilesSet, get: ProjectFilesGet) =>
  async (projectId: string) => {
    const ids = Array.from(get().selectedFileIds)
    if (ids.length === 0) return
    set({ detaching: true, error: null })
    for (const fileId of ids) {
      try {
        await ApiClient.File.delete({ file_id: fileId })
        await emitProjectFileDetached(projectId, fileId)
      } catch (error) {
        set({
          error:
            error instanceof Error
              ? error.message
              : `Failed to delete ${fileId}`,
        })
      }
    }
    set(state => {
      state.detaching = false
      state.selectedFileIds.clear()
    })
  }
