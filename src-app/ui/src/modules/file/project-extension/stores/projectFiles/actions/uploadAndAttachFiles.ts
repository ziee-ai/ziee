import { ApiClient } from '@/api-client'
import { emitProjectFileAttached } from '@/modules/file/project-extension/events'
import { MAX_FILE_UPLOAD_BYTES as MAX_FILE_SIZE } from '@/modules/file/constants'
import type { ProjectFilesGet, ProjectFilesSet } from '../state'
import loadFilesFactory from './_loadFiles'

export default (set: ProjectFilesSet, get: ProjectFilesGet) => {
  const _loadFiles = loadFilesFactory(set, get)
  return async (projectId: string, files: File[]) => {
    let anySucceeded = false
    await Promise.all(
      files.map(async file => {
        const uploadId = `up_${Date.now()}_${Math.random().toString(36).slice(2, 11)}`
        set(state => {
          state.uploadingFiles.set(uploadId, {
            id: uploadId,
            filename: file.name,
            size: file.size,
            progress: 0,
            status: 'pending',
          })
        })

        try {
          if (file.size > MAX_FILE_SIZE) {
            throw new Error(`${file.name} exceeds the per-file size cap`)
          }
          set(state => {
            const entry = state.uploadingFiles.get(uploadId)
            if (entry) entry.status = 'uploading'
          })

          const formData = new FormData()
          formData.append('id', projectId)
          formData.append('file', file)

          const uploaded = await ApiClient.Project.uploadAndAttachFile(
            formData as unknown as { id: string } & FormData,
            {
              fileUploadProgress: {
                onProgress: progress => {
                  set(state => {
                    const entry = state.uploadingFiles.get(uploadId)
                    if (entry) entry.progress = progress
                  })
                },
              },
            },
          )

          set(state => {
            state.uploadingFiles.delete(uploadId)
          })
          anySucceeded = true
          await emitProjectFileAttached(projectId, uploaded.id)
        } catch (error) {
          set(state => {
            const entry = state.uploadingFiles.get(uploadId)
            if (entry) {
              entry.status = 'error'
              entry.error =
                error instanceof Error ? error.message : 'Upload failed'
            }
          })
        }
      }),
    )
    if (anySucceeded) {
      await _loadFiles(projectId)
    }
  }
}
