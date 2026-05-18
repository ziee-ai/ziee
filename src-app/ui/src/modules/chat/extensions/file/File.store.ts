import { createExtensionStore } from '@/modules/chat/core/extensions'
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

  /**
   * IDs of files restored from an existing message during edit mode.
   * These files already exist on the server and must NOT be deleted when
   * removed from the selection or when the edit session ends.
   * Only files uploaded in the current editing session are subject to server deletion.
   */
  restoredFileIds: Set<string>

  // Backup state (for error recovery)
  backupSelectedFiles: Map<string, FileEntity> | null
  backupUploadingFiles: Map<string, FileUploadProgress> | null

  // Actions
  uploadFiles: (files: File[]) => Promise<void>
  removeFile: (fileId: string) => void
  removeUploadingFile: (progressId: string) => void
  clearFiles: () => void
  getFileIds: () => string[]
  getFiles: () => FileEntity[]
  isUploading: () => boolean

  /**
   * Restore files from an existing message into the selection.
   * Marks them as restored so they are not deleted from the server
   * if the user removes them or cancels the edit.
   */
  restoreFilesFromEdit: (files: FileEntity[]) => void

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
    restoredFileIds: new Set(),
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
      const isRestored = get().restoredFileIds.has(fileId)
      set((state) => {
        const newSelected = new Map(state.selectedFiles)
        newSelected.delete(fileId)
        state.selectedFiles = newSelected
      })
      // Only delete from server if the file was uploaded in this session,
      // not if it was restored from an existing message
      if (!isRestored) {
        // Server deletion (if applicable) would go here
        console.log(`[FileStore] Removed non-restored file from selection: ${fileId}`)
      } else {
        console.log(`[FileStore] Removed restored file from selection (not deleted from server): ${fileId}`)
      }
    },

    // Remove an uploading file (cancel)
    removeUploadingFile: (progressId: string) => {
      set((state) => {
        const newUploading = new Map(state.uploadingFiles)
        newUploading.delete(progressId)
        state.uploadingFiles = newUploading
      })
    },

    // Clear all files (called after message send or edit cancel)
    // Only files that are NOT in restoredFileIds would be subject to server deletion
    clearFiles: () => {
      console.log('[FileStore] clearFiles() called')
      const restoredIds = get().restoredFileIds
      const sessionFileIds = [...get().selectedFiles.keys()].filter(
        id => !restoredIds.has(id)
      )
      if (sessionFileIds.length > 0) {
        // Server deletion for session-uploaded files would go here
        console.log('[FileStore] Session files cleared (server deletion if applicable):', sessionFileIds)
      }
      set((state) => {
        state.selectedFiles = new Map()
        state.uploadingFiles = new Map()
        state.restoredFileIds = new Set()
      })
    },

    // Restore files from an existing message into the current selection.
    // Marks them as restored so they are exempt from server deletion.
    restoreFilesFromEdit: (files: FileEntity[]) => {
      set((state) => {
        const newSelected = new Map(state.selectedFiles)
        const newRestoredIds = new Set<string>()
        for (const file of files) {
          newSelected.set(file.id, file)
          newRestoredIds.add(file.id)
        }
        state.selectedFiles = newSelected
        state.restoredFileIds = newRestoredIds
      })
      console.log(`[FileStore] Restored ${files.length} file(s) from edit message`)
    },

    // Get array of file IDs for request composition
    getFileIds: () => {
      return Array.from(get().selectedFiles.keys())
    },

    // Get array of file entities (safe to call outside React components)
    getFiles: () => {
      return Array.from(get().selectedFiles.values())
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
