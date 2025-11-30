import { createExtensionStore } from '../../core/extensions'
import { ApiClient } from '@/api-client'
import type { File as FileEntity } from '@/api-client/types'

/**
 * File upload progress tracking
 */
export interface FileUploadProgress {
  id: string // Temporary ID for progress tracking
  filename: string
  progress: number // 0-100
  status: 'pending' | 'uploading' | 'completed' | 'error'
  error?: string
  size: number
}

/**
 * File Extension Store State
 * Manages file uploads and selection for message attachments
 */
interface FileExtensionStore {
  // File tracking
  uploadingFiles: Map<string, FileUploadProgress>
  selectedFiles: Map<string, FileEntity>

  // Backup state (for error recovery)
  backupSelectedFiles: Map<string, FileEntity> | null
  backupUploadingFiles: Map<string, FileUploadProgress> | null

  // Actions
  uploadFiles: (files: File[]) => Promise<void>
  removeFile: (fileId: string) => void
  removeUploadingFile: (progressId: string) => void
  clearFiles: () => void
  getFileIds: () => string[]
  isUploading: () => boolean

  // Backup/restore methods
  setBackupFiles: () => void
  getBackupFiles: () => { selectedFiles: Map<string, FileEntity>; uploadingFiles: Map<string, FileUploadProgress> } | null
  restoreFromBackup: () => void
  clearBackup: () => void
}

/**
 * Create File Extension Store
 */
export const createFileExtensionStore = () =>
  createExtensionStore<FileExtensionStore>((set, get) => ({
    // Initial state
    uploadingFiles: new Map(),
    selectedFiles: new Map(),
    backupSelectedFiles: null,
    backupUploadingFiles: null,

    // Upload files with progress tracking
    uploadFiles: async (files: File[]) => {
      const uploadPromises = files.map(async (file) => {
        // Generate temporary ID for progress tracking
        const progressId = `upload_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`

        // Create progress entry
        const progress: FileUploadProgress = {
          id: progressId,
          filename: file.name,
          progress: 0,
          status: 'pending',
          size: file.size,
        }

        // Add to uploading files
        set((state) => {
          const newUploading = new Map(state.uploadingFiles)
          newUploading.set(progressId, progress)
          state.uploadingFiles = newUploading
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

          // Upload completed - move to selected files
          set((state) => {
            const newUploading = new Map(state.uploadingFiles)
            const newSelected = new Map(state.selectedFiles)

            newUploading.delete(progressId)
            newSelected.set(uploadedFile.id, uploadedFile)

            console.log(
              `[FileStore] File uploaded: ${uploadedFile.filename} (${uploadedFile.id})`,
            )
            console.log(
              '[FileStore] Selected files:',
              Array.from(newSelected.keys()),
            )

            state.uploadingFiles = newUploading
            state.selectedFiles = newSelected
          })
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
    },

    // Remove a selected file
    removeFile: (fileId: string) => {
      set((state) => {
        const newSelected = new Map(state.selectedFiles)
        newSelected.delete(fileId)
        state.selectedFiles = newSelected
      })
    },

    // Remove an uploading file (cancel)
    removeUploadingFile: (progressId: string) => {
      set((state) => {
        const newUploading = new Map(state.uploadingFiles)
        newUploading.delete(progressId)
        state.uploadingFiles = newUploading
      })
    },

    // Clear all files (called after message send)
    clearFiles: () => {
      console.log('[FileStore] clearFiles() called')
      console.trace('[FileStore] clearFiles stack trace')
      set((state) => {
        state.selectedFiles = new Map()
        state.uploadingFiles = new Map()
      })
    },

    // Get array of file IDs for request composition
    getFileIds: () => {
      return Array.from(get().selectedFiles.keys())
    },

    // Check if any files are currently uploading
    isUploading: () => {
      const uploadingFiles = get().uploadingFiles
      return Array.from(uploadingFiles.values()).some(
        file => file.status === 'pending' || file.status === 'uploading'
      )
    },

    // Backup current files (before clearing)
    setBackupFiles: () => {
      const { selectedFiles, uploadingFiles } = get()
      set((state) => {
        state.backupSelectedFiles = new Map(selectedFiles)
        state.backupUploadingFiles = new Map(uploadingFiles)
      })
      console.log('[FileStore] Backed up files')
    },

    // Get backup files
    getBackupFiles: () => {
      const { backupSelectedFiles, backupUploadingFiles } = get()
      if (!backupSelectedFiles || !backupUploadingFiles) {
        return null
      }
      return {
        selectedFiles: backupSelectedFiles,
        uploadingFiles: backupUploadingFiles,
      }
    },

    // Restore files from backup
    restoreFromBackup: () => {
      const backup = get().getBackupFiles()
      if (backup) {
        set((state) => {
          state.selectedFiles = new Map(backup.selectedFiles)
          state.uploadingFiles = new Map(backup.uploadingFiles)
        })
        console.log('[FileStore] Restored files from backup')
      }
    },

    // Clear backup
    clearBackup: () => {
      set((state) => {
        state.backupSelectedFiles = null
        state.backupUploadingFiles = null
      })
      console.log('[FileStore] Cleared file backup')
    },
  }))

/**
 * Augment ChatExtensionStores with FileStore
 */
declare module '../../types' {
  interface ChatExtensionStores {
    FileStore: ReturnType<typeof createFileExtensionStore>
  }
}
