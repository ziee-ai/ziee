import { ApiClient } from '@/api-client'
import type { KnowledgeBaseDetailGet, KnowledgeBaseDetailSet } from '../state'
import loadDocumentsFactory from './loadDocuments'
import refreshKbFactory from './refreshKb'

/** 100 MiB — mirrors the file module's per-file cap (ProjectFilesManagePanel). */
const MAX_FILE_SIZE = 100 * 1024 * 1024

export default (set: KnowledgeBaseDetailSet, get: KnowledgeBaseDetailGet) => {
  const loadDocuments = loadDocumentsFactory(set, get)
  const refreshKb = refreshKbFactory(set, get)
  return async (id: string, files: File[]) => {
    set(draft => {
      draft.uploading = true
    })
    const fileIds: string[] = []
    try {
      await Promise.all(
        files.map(async file => {
          const uploadId = `up_${Date.now()}_${Math.random().toString(36).slice(2, 11)}`
          set(draft => {
            draft.uploadingFiles.set(uploadId, {
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
            set(draft => {
              const entry = draft.uploadingFiles.get(uploadId)
              if (entry) entry.status = 'uploading'
            })
            const formData = new FormData()
            formData.append('file', file)
            const uploaded = await ApiClient.File.upload(
              formData as unknown as FormData,
              {
                fileUploadProgress: {
                  onProgress: progress => {
                    set(draft => {
                      const entry = draft.uploadingFiles.get(uploadId)
                      if (entry) entry.progress = progress
                    })
                  },
                },
              },
            )
            fileIds.push(uploaded.id)
            set(draft => {
              draft.uploadingFiles.delete(uploadId)
            })
          } catch (error) {
            set(draft => {
              const entry = draft.uploadingFiles.get(uploadId)
              if (entry) {
                entry.status = 'error'
                entry.error =
                  error instanceof Error ? error.message : 'Upload failed'
              }
            })
          }
        }),
      )

      if (fileIds.length === 0) {
        return { attached: 0, skipped_duplicates: 0 }
      }
      const result = await ApiClient.KnowledgeBase.attachDocuments({
        id,
        file_ids: fileIds,
      })
      await loadDocuments(id)
      await refreshKb(id)
      return result
    } finally {
      set(draft => {
        draft.uploading = false
      })
    }
  }
}
