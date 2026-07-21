import { ApiClient } from '@/api-client'
import type { FileGet, FileSet } from '../state'

/** Upload files with progress tracking (into the given pane's buffer, ITEM-32). */
export default (set: FileSet, get: FileGet) => async (paneKey: string, files: File[]) => {
  const uploadPromises = files.map(async (file) => {
    // Generate temporary ID for progress tracking
    const progressId = `upload_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`

    // Create progress entry
    const progress = {
      id: progressId,
      filename: file.name,
      progress: 0,
      status: 'pending' as const,
      size: file.size,
      rawFile: file,
    }

    // Add to uploading files (owned by this pane)
    set((state) => {
      const newUploading = new Map(state.uploadingFiles)
      newUploading.set(progressId, progress)
      state.uploadingFiles = newUploading
      const newOwner = new Map(state.uploadOwner)
      newOwner.set(progressId, paneKey)
      state.uploadOwner = newOwner
    })

    try {
      // Update to uploading status
      set((state) => {
        const newUploading = new Map(state.uploadingFiles)
        const existing = newUploading.get(progressId)
        if (existing) {
          newUploading.set(progressId, { ...existing, status: 'uploading' })
        }
        state.uploadingFiles = newUploading
      })

      // Create FormData
      const formData = new FormData()
      formData.append('file', file)

      // Upload file with progress tracking
      const uploadedFile = await ApiClient.File.upload(formData, {
        fileUploadProgress: {
          onProgress: (progress) => {
            set((state) => {
              const newUploading = new Map(state.uploadingFiles)
              const existing = newUploading.get(progressId)
              if (existing) {
                newUploading.set(progressId, {
                  ...existing,
                  progress,
                })
              }
              state.uploadingFiles = newUploading
            })
          },
        },
      })

      // Upload completed - move to selected files (transfer pane ownership)
      set((state) => {
        const newUploading = new Map(state.uploadingFiles)
        const newSelected = new Map(state.selectedFiles)

        newUploading.delete(progressId)
        newSelected.set(uploadedFile.id, uploadedFile)

        state.uploadingFiles = newUploading
        state.selectedFiles = newSelected

        const newUploadOwner = new Map(state.uploadOwner)
        newUploadOwner.delete(progressId)
        state.uploadOwner = newUploadOwner
        const newFileOwner = new Map(state.fileOwner)
        newFileOwner.set(uploadedFile.id, paneKey)
        state.fileOwner = newFileOwner
      })

      // Trigger thumbnail loading if the uploaded file has a preview
      if (uploadedFile.has_thumbnail && uploadedFile.preview_page_count > 0) {
        get().loadThumbnail(uploadedFile.id)
      }
    } catch (error) {
      // Upload failed
      set((state) => {
        const newUploading = new Map(state.uploadingFiles)
        const existing = newUploading.get(progressId)
        if (existing) {
          newUploading.set(progressId, {
            ...existing,
            status: 'error',
            error: error instanceof Error ? error.message : 'Upload failed',
          })
        }
        state.uploadingFiles = newUploading
      })
      console.error(`Failed to upload file ${file.name}:`, error)
    }
  })

  // Wait for all uploads to complete
  await Promise.all(uploadPromises)
}
