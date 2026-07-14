import { defineStore } from '@ziee/framework/store-kit'
import { enableMapSet } from 'immer'
import { ApiClient } from '@/api-client'
import { Stores } from '@ziee/framework/stores'
import type { File as FileEntity } from '@/api-client/types'
import { type ImageViewState, clampScale, zoomStep } from '../viewers/image/zoom'
import type { TabularViewState } from '../viewers/tabular/tableView'

// Enable Map + Set support in Immer (the store uses Map/Set extensively
// for caches and upload tracking).
enableMapSet()

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
  /** The raw browser File, retained so a failed upload can be retried
   *  without the user re-selecting it. */
  rawFile?: File
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
  // Per-file flag: a page-fetch queue is currently draining for this file.
  previewPageLoadingSet: Set<string>
  // Pages already requested (loaded OR queued) per file — dedup so each page is
  // fetched at most once.
  previewPageRequested: Map<string, Set<number>>
  // Pending page numbers to fetch per file, drained ONE AT A TIME (sequential).
  previewPageQueue: Map<string, number[]>
  // Pages whose fetch SETTLED in failure, per file. A failed page stays
  // `requested` (so it isn't auto-retried into an infinite spinner) and lands
  // here so the viewer can render an explicit error/retry slot instead of a
  // spinner that never resolves. `retryPreviewPage` clears the entry.
  previewPageErrors: Map<string, Set<number>>

  // Actions
  uploadFiles: (files: File[]) => Promise<void>
  removeFile: (fileId: string) => void
  removeUploadingFile: (progressId: string) => void
  retryUpload: (progressId: string) => Promise<void>
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

  // ── Viewer affordance state (viewer-shell-affordances) ──────────────────────
  // Per-file, ephemeral (in-memory), header↔body coordination — mirrors the
  // fileViewModes idiom exactly (immutable-copy setters; dropped on sync, cleared
  // on reconnect). Absent entry ⇒ the documented default, which reproduces the
  // pre-feature render.

  /** Image zoom + fit-mode per file. Default (absent) = { scale: 1, mode: 'fit' }. */
  imageViewStates: Map<string, ImageViewState>
  /** Whether the find-in-document bar is open for a file. Default = false. */
  fileFindOpen: Map<string, boolean>
  /** External find query (e.g. a KB citation passage) to scroll+highlight to. */
  fileFindQuery: Map<string, string>
  /** Whether word-wrap is on for a file's raw/code view. Default = false. */
  fileWordWrap: Map<string, boolean>
  /** The tabular viewer's current view snapshot (filtered/sorted rows, visible
   *  columns, delimiter, selection-as-TSV), published by the body so the
   *  file-viewer header's Export / Copy-selection can act on the current view.
   *  Absent = the body hasn't published (header actions disabled). */
  fileTabularView: Map<string, TabularViewState>

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

  /** Sets the image fit-mode ('fit' | 'actual') for a file, adjusting scale:
   *  'fit' resets to scale 1, 'actual' keeps the current (or 1) scale. */
  setImageViewMode: (fileId: string, mode: 'fit' | 'actual') => void
  /** Multiplies the file's image scale by `factor`, clamped to [0.1, 8], and
   *  switches the mode to 'actual' (any non-fit zoom is an explicit scale). */
  zoomImage: (fileId: string, factor: number) => void
  /** Resets a file's image view to the default { scale: 1, mode: 'fit' }. */
  resetImageView: (fileId: string) => void
  /** Opens/closes the find-in-document bar for a file. */
  setFileFindOpen: (fileId: string, open: boolean) => void
  /** Set the external find query for a file (opens find + scrolls to it). */
  setFileFindQuery: (fileId: string, query: string) => void
  /** Turns word-wrap on/off for a file's raw/code view. */
  setFileWordWrap: (fileId: string, on: boolean) => void
  /** Publishes the tabular body's current view snapshot for the header's
   *  Export / Copy-selection actions. */
  setFileTabularView: (fileId: string, view: TabularViewState) => void
  /** Drops a file's tabular snapshot (e.g. on table unmount / switch to raw
   *  view) so the header's Export / Copy-selection disable rather than act on a
   *  stale, no-longer-rendered view. */
  clearFileTabularView: (fileId: string) => void

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
   * Returns the cached preview page URLs for a file, or a null-filled array.
   * PURE (no side effect): pages are loaded on demand via `requestPreviewPage`
   * as the viewer scrolls, not all at once.
   */
  getPreviewPageUrls: (file: FileEntity) => (string | null)[]

  /**
   * Request a single 1-based preview page. Deduped (each page fetched once) and
   * enqueued into a per-file queue drained sequentially (one request at a time).
   * The viewer calls this for the visible page + the next 2.
   */
  requestPreviewPage: (file: FileEntity, page: number) => void

  /** Clear a page's error + requested state and re-request it (manual retry
   *  from the viewer's error slot). */
  retryPreviewPage: (file: FileEntity, page: number) => void

  /** Internal: drains a file's page queue one request at a time. */
  processPreviewQueue: (file: FileEntity) => Promise<void>

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

  __init__: { __store__: () => void }
  __destroy__: () => void
}

/**
 * File store — chat-composer upload buffer + persistent file caches
 * (thumbnails, previews, content, view modes). Lives at Stores.File
 * (registered in modules/file/module.tsx). Prior name was
 * Stores.Chat.FileStore (nested via the chat-extension framework);
 * relocated out so file-domain state lives in the file module that
 * owns it.
 *
 * Lifecycle:
 *   - Ephemeral upload buffer (uploadingFiles, selectedFiles,
 *     restoredFileIds): cleared on conversation change by the
 *     chat-extension's initialize() hook (explicit
 *     useChatStore.subscribe replaces the chat-extension-framework
 *     auto-scoping that createExtensionStore used to provide).
 *   - Persistent caches (messageFilesCache, thumbnailUrls,
 *     previewPageUrls, fileTextContents, fileBinaryContents,
 *     fileViewModes): survive across conversations — keyed by message
 *     or file id, useful in message-history rendering across the app.
 */
export const File = defineStore('File', {
  immer: true,
  state: {
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
    previewPageRequested: new Map(),
    previewPageQueue: new Map(),
    previewPageErrors: new Map(),
    // Per-ID content cache for right panel tabs
    fileTextContents: new Map(),
    fileTextLoadingSet: new Set(),
    fileBinaryContents: new Map(),
    fileBinaryLoadingSet: new Set(),
    fileViewModes: new Map(),
    imageViewStates: new Map(),
    fileFindOpen: new Map(),
    fileFindQuery: new Map(),
    fileWordWrap: new Map(),
    fileTabularView: new Map(),
  } as unknown as Pick<
    FileExtensionStore,
    | 'uploadingFiles'
    | 'selectedFiles'
    | 'restoredFileIds'
    | 'backupSelectedFiles'
    | 'backupUploadingFiles'
    | 'messageFilesCache'
    | 'messageFilesLoadingSet'
    | 'thumbnailUrls'
    | 'thumbnailLoadingSet'
    | 'previewPageUrls'
    | 'previewPageLoadingSet'
    | 'previewPageRequested'
    | 'previewPageQueue'
    | 'previewPageErrors'
    | 'fileTextContents'
    | 'fileTextLoadingSet'
    | 'fileBinaryContents'
    | 'fileBinaryLoadingSet'
    | 'fileViewModes'
    | 'imageViewStates'
    | 'fileFindOpen'
    | 'fileFindQuery'
    | 'fileWordWrap'
    | 'fileTabularView'
  >,
  actions: (set, getRaw) => {
    const get = getRaw as () => FileExtensionStore
    return {

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
          rawFile: file,
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
      // Revoke + evict the thumbnail/preview blob URLs ONLY for files uploaded
      // in THIS session. A RESTORED file's thumbnails belong to the persistent
      // message-display cache (the same Map feeds the message bubble's image) —
      // revoking/evicting them would break / force-refetch the image still shown
      // in that message. Restored files keep their cached URLs.
      if (!isRestored) {
        const thumbnailUrl = get().thumbnailUrls.get(fileId)
        if (thumbnailUrl) URL.revokeObjectURL(thumbnailUrl)
        const pageUrls = get().previewPageUrls.get(fileId)
        if (pageUrls) pageUrls.forEach(url => url && URL.revokeObjectURL(url))
      }
      set((state) => {
        const newSelected = new Map(state.selectedFiles)
        newSelected.delete(fileId)
        state.selectedFiles = newSelected
        if (!isRestored) {
          const newThumbnails = new Map(state.thumbnailUrls)
          newThumbnails.delete(fileId)
          const newLoadingSet = new Set(state.thumbnailLoadingSet)
          newLoadingSet.delete(fileId)
          const newPageUrls = new Map(state.previewPageUrls)
          newPageUrls.delete(fileId)
          const newPageLoadingSet = new Set(state.previewPageLoadingSet)
          newPageLoadingSet.delete(fileId)
          state.thumbnailUrls = newThumbnails
          state.thumbnailLoadingSet = newLoadingSet
          state.previewPageUrls = newPageUrls
          state.previewPageLoadingSet = newPageLoadingSet
        }
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

    // Retry a failed upload: drop the errored entry and re-run the upload for
    // its retained raw File, producing a fresh progress entry. No-op if the
    // entry is missing or the raw File wasn't retained.
    retryUpload: async (progressId: string) => {
      const entry = get().uploadingFiles.get(progressId)
      if (!entry?.rawFile) return
      get().removeUploadingFile(progressId)
      await get().uploadFiles([entry.rawFile])
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
      // Revoke + evict thumbnail/preview blob URLs ONLY for session-uploaded
      // files. RESTORED files' thumbnails belong to the persistent
      // message-display cache (shared Map) — revoking/evicting them would
      // break / force-refetch the image still shown in that message bubble.
      const thumbnailUrls = get().thumbnailUrls
      for (const fileId of sessionFileIds) {
        const url = thumbnailUrls.get(fileId)
        if (url) URL.revokeObjectURL(url)
      }
      const previewPageUrls = get().previewPageUrls
      for (const fileId of sessionFileIds) {
        const pages = previewPageUrls.get(fileId)
        if (pages) pages.forEach(url => url && URL.revokeObjectURL(url))
      }
      set((state) => {
        const newThumbnails = new Map(state.thumbnailUrls)
        for (const fileId of sessionFileIds) newThumbnails.delete(fileId)
        const newLoadingSet = new Set(state.thumbnailLoadingSet)
        for (const fileId of sessionFileIds) newLoadingSet.delete(fileId)
        const newPageUrls = new Map(state.previewPageUrls)
        for (const fileId of sessionFileIds) newPageUrls.delete(fileId)
        const newPageLoadingSet = new Set(state.previewPageLoadingSet)
        for (const fileId of sessionFileIds) newPageLoadingSet.delete(fileId)
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
        // MERGE into the existing restored set (symmetric with `newSelected`).
        // The two-phase edit-restore flow calls this twice: Phase 1 with all
        // stubs, then Phase 2 with only the successfully-fetched `validFiles`
        // (filtered). Replacing here would drop protection for any file whose
        // Phase-2 fetch failed, exposing it to server deletion. A fresh edit
        // session resets the set via `clearFiles()`.
        const newRestoredIds = new Set(state.restoredFileIds)
        for (const file of files) {
          // The composer always holds the HEAD entity (version ==
          // current_version_id). It shows/sends the file's current state;
          // version PINNING is a property of already-SENT message blocks
          // (rendered by FileAttachmentRenderer), not the composer.
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

    setImageViewMode: (fileId: string, mode: 'fit' | 'actual') => {
      set((state) => {
        const next = new Map(state.imageViewStates)
        const cur = next.get(fileId) ?? { scale: 1, mode: 'fit' as const }
        // 'fit' pins scale back to 1 (object-contain); 'actual' keeps the
        // current scale (or 1 if it was still at fit).
        next.set(fileId, { mode, scale: mode === 'fit' ? 1 : clampScale(cur.scale) })
        state.imageViewStates = next
      })
    },

    zoomImage: (fileId: string, factor: number) => {
      set((state) => {
        const next = new Map(state.imageViewStates)
        const cur = next.get(fileId) ?? { scale: 1, mode: 'fit' as const }
        next.set(fileId, { mode: 'actual', scale: zoomStep(cur.scale, factor) })
        state.imageViewStates = next
      })
    },

    resetImageView: (fileId: string) => {
      set((state) => {
        const next = new Map(state.imageViewStates)
        next.set(fileId, { scale: 1, mode: 'fit' })
        state.imageViewStates = next
      })
    },

    setFileFindOpen: (fileId: string, open: boolean) => {
      set((state) => {
        const next = new Map(state.fileFindOpen)
        next.set(fileId, open)
        state.fileFindOpen = next
      })
    },

    setFileFindQuery: (fileId: string, query: string) => {
      set((state) => {
        const nq = new Map(state.fileFindQuery)
        nq.set(fileId, query)
        state.fileFindQuery = nq
        const no = new Map(state.fileFindOpen)
        no.set(fileId, true)
        state.fileFindOpen = no
      })
    },

    setFileWordWrap: (fileId: string, on: boolean) => {
      set((state) => {
        const next = new Map(state.fileWordWrap)
        next.set(fileId, on)
        state.fileWordWrap = next
      })
    },

    setFileTabularView: (fileId: string, view: TabularViewState) => {
      set((state) => {
        const next = new Map(state.fileTabularView)
        next.set(fileId, view)
        state.fileTabularView = next
      })
    },

    clearFileTabularView: (fileId: string) => {
      set((state) => {
        if (!state.fileTabularView.has(fileId)) return
        const next = new Map(state.fileTabularView)
        next.delete(fileId)
        state.fileTabularView = next
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
      const { getAuthToken } = await import('@ziee/framework/api-client/core')
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
          // Only cache a well-formed entity. A malformed response (missing id
          // — e.g. a transient backend hiccup) must not poison the cache over
          // the caller-supplied fallback (the content-block-derived entity that
          // already carries filename + size); caching it would surface blanks /
          // "NaN" size in its place.
          if (fileInfo && (fileInfo as { id?: string }).id) {
            newCache.set(fileId, fileInfo)
          }
          const newSet = new Set(state.messageFilesLoadingSet)
          newSet.delete(fileId)
          state.messageFilesCache = newCache
          state.messageFilesLoadingSet = newSet
        })
        // NOTE: thumbnails are intentionally NOT eager-loaded here. The
        // consumers that actually display one (FileCard, ImageBody) call
        // getThumbnailUrl() when they render. This lets viewport-gated inline
        // previews avoid fetching/decoding thumbnails for off-screen files on
        // reload — the fix for laggy reloads with many inline images.
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
      // Pure read — no auto-load. The viewer drives loading via
      // requestPreviewPage as pages scroll into view.
      return (
        get().previewPageUrls.get(file.id) ??
        Array(file.preview_page_count).fill(null)
      )
    },

    requestPreviewPage: (file: FileEntity, page: number): void => {
      if (page < 1 || page > file.preview_page_count) return
      if (get().previewPageRequested.get(file.id)?.has(page)) return

      set((state) => {
        const reqMap = new Map(state.previewPageRequested)
        const reqSet = new Set(reqMap.get(file.id) ?? [])
        reqSet.add(page)
        reqMap.set(file.id, reqSet)
        state.previewPageRequested = reqMap

        const qMap = new Map(state.previewPageQueue)
        qMap.set(file.id, [...(qMap.get(file.id) ?? []), page])
        state.previewPageQueue = qMap

        if (!state.previewPageUrls.has(file.id)) {
          const m = new Map(state.previewPageUrls)
          m.set(file.id, Array(file.preview_page_count).fill(null))
          state.previewPageUrls = m
        }
      })

      void get().processPreviewQueue(file)
    },

    retryPreviewPage: (file: FileEntity, page: number): void => {
      // Clear the settled error + the requested/loaded mark so requestPreviewPage
      // enqueues a fresh attempt.
      set((state) => {
        const errMap = new Map(state.previewPageErrors)
        const errSet = new Set(errMap.get(file.id) ?? [])
        errSet.delete(page)
        errMap.set(file.id, errSet)
        state.previewPageErrors = errMap

        const reqMap = new Map(state.previewPageRequested)
        const reqSet = new Set(reqMap.get(file.id) ?? [])
        reqSet.delete(page)
        reqMap.set(file.id, reqSet)
        state.previewPageRequested = reqMap
      })
      get().requestPreviewPage(file, page)
    },

    processPreviewQueue: async (file: FileEntity): Promise<void> => {
      // One drain per file — a running drain picks up newly-enqueued pages.
      if (get().previewPageLoadingSet.has(file.id)) return
      set((state) => {
        const s = new Set(state.previewPageLoadingSet)
        s.add(file.id)
        state.previewPageLoadingSet = s
      })

      try {
        while ((get().previewPageQueue.get(file.id) ?? []).length > 0) {
          const queue = get().previewPageQueue.get(file.id) ?? []
          const page = queue[0]
          set((state) => {
            const q = new Map(state.previewPageQueue)
            q.set(file.id, queue.slice(1))
            state.previewPageQueue = q
          })

          try {
            const response = await ApiClient.File.getPreview({ file_id: file.id, page })
            const url = URL.createObjectURL(response)
            set((state) => {
              const existing =
                state.previewPageUrls.get(file.id) ??
                Array(file.preview_page_count).fill(null)
              const updated = [...existing]
              updated[page - 1] = url
              const m = new Map(state.previewPageUrls)
              m.set(file.id, updated)
              state.previewPageUrls = m
            })
          } catch (error) {
            // Record the failure so the viewer renders an explicit error/retry
            // slot instead of a spinner that never resolves (the page stays
            // `requested`, so a scroll-triggered re-request won't spin it
            // forever; `retryPreviewPage` is the deliberate re-attempt path).
            set((state) => {
              const errMap = new Map(state.previewPageErrors)
              const errSet = new Set(errMap.get(file.id) ?? [])
              errSet.add(page)
              errMap.set(file.id, errSet)
              state.previewPageErrors = errMap
            })
            console.debug(
              `[FileStore] Failed to load preview page ${page} for ${file.id}:`,
              error,
            )
          }
        }
      } finally {
        set((state) => {
          const s = new Set(state.previewPageLoadingSet)
          s.delete(file.id)
          state.previewPageLoadingSet = s
        })
      }

      // A page enqueued during the flag-teardown window would otherwise strand;
      // restart the drain if the queue refilled.
      if ((get().previewPageQueue.get(file.id) ?? []).length > 0) {
        void get().processPreviewQueue(file)
      }
    },

    loadThumbnail: async (fileId: string): Promise<void> => {
      set((state) => {
        const newSet = new Set(state.thumbnailLoadingSet)
        newSet.add(fileId)
        state.thumbnailLoadingSet = newSet
      })
      try {
        // Use the dedicated ~300px thumbnail (GET /files/{id}/thumbnail), not
        // the full-size preview page 1 (~2000px) — the card image only needs a
        // small image, so this is far lighter to fetch + decode.
        const response = await ApiClient.File.getThumbnail({ file_id: fileId })
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

    }
  },
  init: ({ set, get: getRaw, onCleanup }) => {
    const get = getRaw as () => FileExtensionStore
        const eventBus = Stores.EventBus
        const GROUP = 'FileStore'
        // A file's HEAD changed (restore / MCP edit / sandbox version-back),
        // possibly on another device. The content caches below are keyed by
        // fileId with NO version, so the cached bytes are now stale — drop the
        // affected file's entries so the next viewer render refetches the new
        // HEAD. (Versioning made a fileId's bytes mutable; pre-versioning they
        // were immutable, so caching forever used to be safe.)
        const onFileSync = (event: { data?: { id?: string } }) => {
          const fileId = event?.data?.id
          if (!fileId) return
          const trackHead = get().messageFilesCache.has(fileId)
          const trackSelected = get().selectedFiles.has(fileId)
          set((s) => {
            const t = new Map(s.fileTextContents)
            t.delete(fileId)
            s.fileTextContents = t
            const b = new Map(s.fileBinaryContents)
            b.delete(fileId)
            s.fileBinaryContents = b
            const v = new Map(s.fileViewModes)
            v.delete(fileId)
            s.fileViewModes = v
            // Viewer affordance state is keyed by fileId with no version, so a
            // HEAD change makes a stale zoom/wrap/find-open meaningless — drop it
            // (the viewer re-renders at the documented default).
            const iv = new Map(s.imageViewStates)
            iv.delete(fileId)
            s.imageViewStates = iv
            const fo = new Map(s.fileFindOpen)
            fo.delete(fileId)
            s.fileFindOpen = fo
            const ww = new Map(s.fileWordWrap)
            ww.delete(fileId)
            s.fileWordWrap = ww
            const tv = new Map(s.fileTabularView)
            tv.delete(fileId)
            s.fileTabularView = tv
          })
          // Refresh the cached HEAD entity (version/metadata) so open panels
          // re-render against the new head. Async action → outside set().
          if (trackHead) void get().loadMessageFile(fileId)
          // Keep the composer's entry fresh too — selectedFiles always mirrors
          // head, so an edit/restore on another device must update its metadata
          // (not just the content caches cleared above).
          if (trackSelected) {
            void (async () => {
              try {
                const updated = await ApiClient.File.get({ file_id: fileId })
                set((s) => {
                  if (!s.selectedFiles.has(fileId)) return // removed meanwhile
                  const m = new Map(s.selectedFiles)
                  m.set(fileId, updated)
                  s.selectedFiles = m
                })
              } catch {
                /* best-effort; content caches were already cleared above */
              }
            })()
          }
        }
        // Reconnect may have dropped events — clear ALL content caches so every
        // open viewer refetches.
        const onReconnect = () => {
          set((s) => {
            s.fileTextContents = new Map()
            s.fileBinaryContents = new Map()
            s.fileViewModes = new Map()
            s.imageViewStates = new Map()
            s.fileFindOpen = new Map()
            s.fileWordWrap = new Map()
            s.fileTabularView = new Map()
          })
        }
        eventBus.on('sync:file', onFileSync, GROUP)
        eventBus.on('sync:reconnect', onReconnect, GROUP)
    onCleanup(() => {
      Stores.EventBus.removeGroupListeners('FileStore')
    })
  },
})

export const useFileStore = File.store
