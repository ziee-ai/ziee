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

  // Cache for file entities shown in message history (fetched on demand)
  messageFilesCache: Map<string, FileEntity>
  messageFilesLoadingSet: Set<string>

  // Cache for thumbnail blob URLs (shared across selectedFiles + messageFilesCache)
  thumbnailUrls: Map<string, string>
  thumbnailLoadingSet: Set<string>

  // Cache for all preview page blob URLs (used for PDF scroll view)
  // Map<fileId, Array<blobUrl|null>> — null means that page hasn't loaded yet
  previewPageUrls: Map<string, (string | null)[]>
  previewPageLoadingSet: Set<string>

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

  // Per-ID content cache for right panel tabs (supports multiple open tabs)
  fileTextContents: Map<string, string>
  fileTextLoadingSet: Set<string>
  fileBinaryContents: Map<string, ArrayBuffer | null>
  fileBinaryLoadingSet: Set<string>
  fileViewModes: Map<string, 'compiled' | 'raw'>

  /**
   * Returns cached text content for the file. Triggers load on first call.
   * Returns null while loading. Call from render — no useEffect needed.
   * Pass the file entity to avoid a race condition when messageFilesCache hasn't loaded yet.
   */
  getFileTextContent: (fileId: string, file?: FileEntity) => string | null

  /**
   * Async action: fetches text/html/svg content and stores in fileTextContents.
   */
  loadFileTextContent: (fileId: string, file?: FileEntity) => Promise<void>

  /**
   * Returns cached binary content for the file. Triggers load on first call.
   * Returns null while loading. Only populated for binary formats (e.g. xlsx).
   * Pass the file entity to avoid a race condition when messageFilesCache hasn't loaded yet.
   */
  getFileBinaryContent: (fileId: string, file?: FileEntity) => ArrayBuffer | null

  /**
   * Async action: fetches binary content and stores in fileBinaryContents.
   */
  loadFileBinaryContent: (fileId: string, file?: FileEntity) => Promise<void>

  /** Sets the view mode (compiled/raw) for a specific file tab. */
  setFileViewMode: (fileId: string, mode: 'compiled' | 'raw') => void

  /**
   * Returns the cached file entity for a message file, or the fallback if not yet loaded.
   * Triggers async loading in the background on first call for a given fileId.
   * Components call this directly (no useEffect needed) — store handles re-renders.
   */
  getMessageFile: (fileId: string, fallback: FileEntity) => FileEntity

  /**
   * Async action: fetches full file entity and updates messageFilesCache.
   */
  loadMessageFile: (fileId: string) => Promise<void>

  /**
   * One-shot fetch returning the full FileEntity for a file id.
   * Used by extension hooks (e.g. edit-conversation restore) that
   * need a Promise rather than store-cache-backed state. Does NOT
   * update messageFilesCache.
   */
  getFileEntityById: (fileId: string) => Promise<FileEntity>

  /**
   * Fetch the raw text body at a dynamic same-origin /api/... URL —
   * used by `useResourceLinkContent` for inline MCP resource_link
   * blocks whose targets aren't known endpoints in ApiClient.
   * Attaches the bearer token from the auth store so the request
   * matches authentication used everywhere else (the previous
   * inlined `fetch(url)` was unauthenticated).
   */
  fetchResourceLinkText: (url: string) => Promise<string>

  /**
   * Returns the cached thumbnail blob URL for a file, or null if not yet loaded.
   * Triggers async loading when the file has has_thumbnail=true and preview_page_count>0.
   * Components call this directly (no useEffect needed) — store handles re-renders.
   * Pass the file entity to avoid a race condition when messageFilesCache hasn't loaded yet.
   */
  getThumbnailUrl: (fileId: string, file?: FileEntity) => string | null

  /**
   * Async action: fetches page-1 preview and stores blob URL in thumbnailUrls.
   */
  loadThumbnail: (fileId: string) => Promise<void>

  /**
   * Returns the cached preview page URLs for a file, or a null-filled array if not yet loaded.
   * Triggers async loading on first call. Components call this directly — store re-renders progressively.
   */
  getPreviewPageUrls: (file: FileEntity) => (string | null)[]

  /**
   * Async action: fetches all preview pages one by one and stores blob URLs.
   * Updates page slots individually so the UI renders progressively.
   */
  loadPreviewPages: (file: FileEntity) => Promise<void>

  /**
   * Triggers a browser download for the given file. Throws on failure.
   */
  downloadFile: (file: FileEntity) => Promise<void>

  /**
   * Opens the file in a new browser tab. Mints a fresh short-lived download
   * token (so the unauthenticated tab navigation still succeeds — a plain
   * `<a target=_blank>` can't send the bearer header) and opens the
   * same-origin `download-with-token` URL. Throws on failure.
   */
  openFileInNewTab: (fileId: string) => Promise<void>
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
    messageFilesCache: new Map(),
    messageFilesLoadingSet: new Set(),
    thumbnailUrls: new Map(),
    thumbnailLoadingSet: new Set(),
    previewPageUrls: new Map(),
    previewPageLoadingSet: new Set(),

    // Per-ID content cache for right panel tabs
    fileTextContents: new Map(),
    fileTextLoadingSet: new Set(),
    fileBinaryContents: new Map(),
    fileBinaryLoadingSet: new Set(),
    fileViewModes: new Map(),

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

            state.uploadingFiles = newUploading
            state.selectedFiles = newSelected
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
    },

    // Remove a selected file
    removeFile: (fileId: string) => {
      const isRestored = get().restoredFileIds.has(fileId)
      // Revoke thumbnail blob URL if present
      const thumbnailUrl = get().thumbnailUrls.get(fileId)
      if (thumbnailUrl) URL.revokeObjectURL(thumbnailUrl)
      // Revoke preview page blob URLs if present
      const pageUrls = get().previewPageUrls.get(fileId)
      if (pageUrls) pageUrls.forEach(url => url && URL.revokeObjectURL(url))
      set((state) => {
        const newSelected = new Map(state.selectedFiles)
        newSelected.delete(fileId)
        const newThumbnails = new Map(state.thumbnailUrls)
        newThumbnails.delete(fileId)
        const newLoadingSet = new Set(state.thumbnailLoadingSet)
        newLoadingSet.delete(fileId)
        const newPageUrls = new Map(state.previewPageUrls)
        newPageUrls.delete(fileId)
        const newPageLoadingSet = new Set(state.previewPageLoadingSet)
        newPageLoadingSet.delete(fileId)
        state.selectedFiles = newSelected
        state.thumbnailUrls = newThumbnails
        state.thumbnailLoadingSet = newLoadingSet
        state.previewPageUrls = newPageUrls
        state.previewPageLoadingSet = newPageLoadingSet
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
      const selectedIds = [...get().selectedFiles.keys()]
      const sessionFileIds = selectedIds.filter(id => !restoredIds.has(id))
      if (sessionFileIds.length > 0) {
        // Server deletion for session-uploaded files would go here
        console.log('[FileStore] Session files cleared (server deletion if applicable):', sessionFileIds)
      }
      // Revoke thumbnail blob URLs for all selected files
      const thumbnailUrls = get().thumbnailUrls
      for (const fileId of selectedIds) {
        const url = thumbnailUrls.get(fileId)
        if (url) URL.revokeObjectURL(url)
      }
      // Revoke preview page blob URLs for all selected files
      const previewPageUrls = get().previewPageUrls
      for (const fileId of selectedIds) {
        const pages = previewPageUrls.get(fileId)
        if (pages) pages.forEach(url => url && URL.revokeObjectURL(url))
      }
      set((state) => {
        const newThumbnails = new Map(state.thumbnailUrls)
        for (const fileId of selectedIds) newThumbnails.delete(fileId)
        const newLoadingSet = new Set(state.thumbnailLoadingSet)
        for (const fileId of selectedIds) newLoadingSet.delete(fileId)
        const newPageUrls = new Map(state.previewPageUrls)
        for (const fileId of selectedIds) newPageUrls.delete(fileId)
        const newPageLoadingSet = new Set(state.previewPageLoadingSet)
        for (const fileId of selectedIds) newPageLoadingSet.delete(fileId)
        state.selectedFiles = new Map()
        state.uploadingFiles = new Map()
        state.restoredFileIds = new Set()
        state.thumbnailUrls = newThumbnails
        state.thumbnailLoadingSet = newLoadingSet
        state.previewPageUrls = newPageUrls
        state.previewPageLoadingSet = newPageLoadingSet
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
      // Trigger thumbnail loading for restored files that have previews
      for (const file of files) {
        if (
          file.has_thumbnail &&
          file.preview_page_count > 0 &&
          !get().thumbnailUrls.has(file.id) &&
          !get().thumbnailLoadingSet.has(file.id)
        ) {
          get().loadThumbnail(file.id)
        }
      }
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

    getFileTextContent: (fileId: string, file?: FileEntity): string | null => {
      const cached = get().fileTextContents.get(fileId)
      if (cached !== undefined) return cached

      if (!get().fileTextLoadingSet.has(fileId)) {
        Promise.resolve().then(() => get().loadFileTextContent(fileId, file))
      }

      return null
    },

    loadFileTextContent: async (fileId: string, fallbackFile?: FileEntity): Promise<void> => {
      if (get().fileTextLoadingSet.has(fileId) || get().fileTextContents.has(fileId)) return

      const file = get().messageFilesCache.get(fileId) ?? get().selectedFiles.get(fileId) ?? fallbackFile
      if (!file) return

      set((state) => {
        const newSet = new Set(state.fileTextLoadingSet)
        newSet.add(fileId)
        state.fileTextLoadingSet = newSet
      })

      try {
        let text = ''
        const e = file.filename.split('.').pop()?.toLowerCase() ?? ''
        const isHtmlOrSvg =
          file.mime_type === 'text/html' || file.mime_type === 'image/svg+xml' ||
          e === 'html' || e === 'htm' || e === 'svg'
        if (isHtmlOrSvg) {
          const response = await ApiClient.File.download({ file_id: file.id })
          const blob = response instanceof Blob ? response : new Blob([response])
          text = await blob.text()
        } else {
          const response = await ApiClient.File.getTextContent({ file_id: file.id })
          text = typeof response === 'string' ? response : await (response as Blob).text()
        }
        set((state) => {
          const newContents = new Map(state.fileTextContents)
          newContents.set(fileId, text)
          const newSet = new Set(state.fileTextLoadingSet)
          newSet.delete(fileId)
          state.fileTextContents = newContents
          state.fileTextLoadingSet = newSet
        })
      } catch (error) {
        set((state) => {
          const newSet = new Set(state.fileTextLoadingSet)
          newSet.delete(fileId)
          state.fileTextLoadingSet = newSet
        })
        console.error('[FileStore] Failed to load file text content:', error)
      }
    },

    getFileBinaryContent: (fileId: string, file?: FileEntity): ArrayBuffer | null => {
      const cached = get().fileBinaryContents.get(fileId)
      if (cached !== undefined) return cached

      if (!get().fileBinaryLoadingSet.has(fileId)) {
        Promise.resolve().then(() => get().loadFileBinaryContent(fileId, file))
      }

      return null
    },

    loadFileBinaryContent: async (fileId: string, fallbackFile?: FileEntity): Promise<void> => {
      if (get().fileBinaryLoadingSet.has(fileId) || get().fileBinaryContents.has(fileId)) return

      const file = get().messageFilesCache.get(fileId) ?? get().selectedFiles.get(fileId) ?? fallbackFile
      if (!file) return

      set((state) => {
        const newSet = new Set(state.fileBinaryLoadingSet)
        newSet.add(fileId)
        state.fileBinaryLoadingSet = newSet
      })

      try {
        const response = await ApiClient.File.download({ file_id: file.id })
        const blob = response instanceof Blob ? response : new Blob([response])
        const buffer = await blob.arrayBuffer()
        set((state) => {
          const newContents = new Map(state.fileBinaryContents)
          newContents.set(fileId, buffer)
          const newSet = new Set(state.fileBinaryLoadingSet)
          newSet.delete(fileId)
          state.fileBinaryContents = newContents
          state.fileBinaryLoadingSet = newSet
        })
      } catch (error) {
        set((state) => {
          const newSet = new Set(state.fileBinaryLoadingSet)
          newSet.delete(fileId)
          state.fileBinaryLoadingSet = newSet
        })
        console.error('[FileStore] Failed to load file binary content:', error)
      }
    },

    setFileViewMode: (fileId: string, mode: 'compiled' | 'raw') => {
      set((state) => {
        const newModes = new Map(state.fileViewModes)
        newModes.set(fileId, mode)
        state.fileViewModes = newModes
      })
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

    getMessageFile: (fileId: string, fallback: FileEntity): FileEntity => {
      const cached = get().messageFilesCache.get(fileId)
      if (!cached && !get().messageFilesLoadingSet.has(fileId)) {
        // Defer to avoid calling set() during React render (would cause React warning)
        Promise.resolve().then(() => get().loadMessageFile(fileId))
      }
      return cached ?? fallback
    },

    getFileEntityById: async (fileId: string): Promise<FileEntity> => {
      return await ApiClient.File.get({ file_id: fileId })
    },

    fetchResourceLinkText: async (url: string): Promise<string> => {
      // Lazy-import to avoid a circular dep with the api-client module
      // (which itself depends on auth-storage parsing — keeping that
      // out of the file-store load order).
      const { getAuthToken } = await import('@/api-client/core')
      const token = getAuthToken()
      const res = await fetch(url, {
        headers: token ? { Authorization: `Bearer ${token}` } : {},
      })
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      return await res.text()
    },

    loadMessageFile: async (fileId: string): Promise<void> => {
      set((state) => {
        const newSet = new Set(state.messageFilesLoadingSet)
        newSet.add(fileId)
        state.messageFilesLoadingSet = newSet
      })
      try {
        const fileInfo = await ApiClient.File.get({ file_id: fileId })
        set((state) => {
          const newCache = new Map(state.messageFilesCache)
          newCache.set(fileId, fileInfo)
          const newSet = new Set(state.messageFilesLoadingSet)
          newSet.delete(fileId)
          state.messageFilesCache = newCache
          state.messageFilesLoadingSet = newSet
        })
        // Trigger thumbnail loading if the file has a preview
        if (fileInfo.has_thumbnail && fileInfo.preview_page_count > 0) {
          get().loadThumbnail(fileId)
        }
      } catch (error) {
        set((state) => {
          const newSet = new Set(state.messageFilesLoadingSet)
          newSet.delete(fileId)
          state.messageFilesLoadingSet = newSet
        })
        console.error(`[FileStore] Failed to load message file ${fileId}:`, error)
      }
    },

    getThumbnailUrl: (fileId: string, fallbackFile?: FileEntity): string | null => {
      const cached = get().thumbnailUrls.get(fileId)
      if (cached) return cached

      if (!get().thumbnailLoadingSet.has(fileId)) {
        const file = get().selectedFiles.get(fileId) ?? get().messageFilesCache.get(fileId) ?? fallbackFile
        if (file?.has_thumbnail && file?.preview_page_count > 0) {
          get().loadThumbnail(fileId)
        }
      }

      return null
    },

    getPreviewPageUrls: (file: FileEntity): (string | null)[] => {
      const cached = get().previewPageUrls.get(file.id)
      if (cached) return cached

      if (!get().previewPageLoadingSet.has(file.id) && file.preview_page_count > 0) {
        Promise.resolve().then(() => get().loadPreviewPages(file))
      }

      // Return a null-filled placeholder array so the component can render spinners
      return Array(file.preview_page_count).fill(null)
    },

    loadPreviewPages: async (file: FileEntity): Promise<void> => {
      set((state) => {
        const newSet = new Set(state.previewPageLoadingSet)
        newSet.add(file.id)
        // Initialise with null slots
        const newPageUrls = new Map(state.previewPageUrls)
        newPageUrls.set(file.id, Array(file.preview_page_count).fill(null))
        state.previewPageLoadingSet = newSet
        state.previewPageUrls = newPageUrls
      })

      const blobUrls: string[] = []
      try {
        for (let page = 1; page <= file.preview_page_count; page++) {
          const response = await ApiClient.File.getPreview({ file_id: file.id, page })
          const url = URL.createObjectURL(response)
          blobUrls.push(url)

          // Update slot-by-slot so the UI renders progressively
          set((state) => {
            const existing = state.previewPageUrls.get(file.id)
            if (!existing) return
            const updated = [...existing]
            updated[page - 1] = url
            const newPageUrls = new Map(state.previewPageUrls)
            newPageUrls.set(file.id, updated)
            state.previewPageUrls = newPageUrls
          })
        }
      } catch (error) {
        console.debug(`[FileStore] Failed to load preview pages for ${file.id}:`, error)
      } finally {
        set((state) => {
          const newSet = new Set(state.previewPageLoadingSet)
          newSet.delete(file.id)
          state.previewPageLoadingSet = newSet
        })
      }
    },

    loadThumbnail: async (fileId: string): Promise<void> => {
      set((state) => {
        const newSet = new Set(state.thumbnailLoadingSet)
        newSet.add(fileId)
        state.thumbnailLoadingSet = newSet
      })
      try {
        const response = await ApiClient.File.getPreview({ file_id: fileId, page: 1 })
        const objectUrl = URL.createObjectURL(response)
        set((state) => {
          const newUrls = new Map(state.thumbnailUrls)
          newUrls.set(fileId, objectUrl)
          const newSet = new Set(state.thumbnailLoadingSet)
          newSet.delete(fileId)
          state.thumbnailUrls = newUrls
          state.thumbnailLoadingSet = newSet
        })
      } catch (error) {
        set((state) => {
          const newSet = new Set(state.thumbnailLoadingSet)
          newSet.delete(fileId)
          state.thumbnailLoadingSet = newSet
        })
        console.debug(`[FileStore] Failed to load thumbnail for ${fileId}:`, error)
      }
    },

    downloadFile: async (file: FileEntity): Promise<void> => {
      const response = await ApiClient.File.download({ file_id: file.id })
      const blob = response instanceof Blob ? response : new Blob([response])
      const url = window.URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = file.filename
      document.body.appendChild(a)
      a.click()
      window.URL.revokeObjectURL(url)
      document.body.removeChild(a)
    },

    openFileInNewTab: async (fileId: string): Promise<void> => {
      // Mint a fresh short-lived token: a new-tab navigation can't carry the
      // bearer header, but the download-with-token endpoint authenticates via
      // the query param. Same-origin relative URL so it works in dev + prod.
      const { token } = await ApiClient.File.generateDownloadToken({ file_id: fileId })
      const url = `/api/files/${fileId}/download-with-token?token=${encodeURIComponent(token)}`
      window.open(url, '_blank', 'noopener,noreferrer')
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
