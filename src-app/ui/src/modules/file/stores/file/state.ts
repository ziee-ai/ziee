import { enableMapSet } from 'immer'
import type { StoreSet } from '@ziee/framework/store-kit'
import type { File as FileEntity } from '@/api-client/types'
import { type ImageViewState } from '../../viewers/image/zoom'
import type { TabularViewState } from '../../viewers/tabular/tableView'
import {
  SINGLE_PANE_KEY,
  composerPaneKey,
} from '../composerOwnership'

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

/** A single pane's compose-time backup (its OWN owned entries only, ITEM-32). */
type PaneBackup = {
  selectedFiles: Map<string, FileEntity>
  uploadingFiles: Map<string, FileUploadProgress>
}

export interface FileExtensionStore {
  // File tracking. The buffers are shared Maps but each entry is OWNED by a pane
  // (fileOwner / uploadOwner, keyed by composerPaneKey) so two split panes keep
  // independent attachments (ITEM-32) — the buffer actions + display filter by
  // the owning pane. The thumbnail/preview caches below stay GLOBAL (id-keyed).
  uploadingFiles: Map<string, FileUploadProgress>
  selectedFiles: Map<string, FileEntity>
  /** selectedFiles fileId → owning pane key (ITEM-32). */
  fileOwner: Map<string, string>
  /** uploadingFiles progressId → owning pane key (ITEM-32). */
  uploadOwner: Map<string, string>

  /**
   * IDs of files restored from an existing message during edit mode.
   * These files already exist on the server and must NOT be deleted when
   * removed from the selection or when the edit session ends.
   * Only files uploaded in the current editing session are subject to server deletion.
   */
  restoredFileIds: Set<string>

  // Backup state (for error recovery), PER-PANE (ITEM-32): each sending pane
  // backs up ONLY its own owned entries into its own slot keyed by composer pane
  // key, so a stream-error restore for one pane never clobbers a concurrently-
  // edited other pane's composer buffer (and two concurrent sends don't overwrite
  // one shared backup slot).
  backupByPane: Map<string, PaneBackup>

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

  // Actions — buffer actions take the composer pane key (ITEM-32).
  uploadFiles: (paneKey: string, files: File[]) => Promise<void>
  removeFile: (fileId: string) => void
  removeUploadingFile: (progressId: string) => void
  retryUpload: (paneKey: string, progressId: string) => Promise<void>
  clearFiles: (paneKey: string) => void
  getFileIds: (paneKey: string) => string[]
  getFiles: (paneKey: string) => FileEntity[]
  isUploading: (paneKey: string) => boolean

  /**
   * Restore files from an existing message into the selection.
   * Marks them as restored so they are not deleted from the server
   * if the user removes them or cancels the edit.
   */
  restoreFilesFromEdit: (paneKey: string, files: FileEntity[]) => void

  // Backup/restore methods (per-pane: pass the SENDING pane's composer key)
  setBackupFiles: (paneKey: string) => void
  restoreFromBackup: (paneKey: string) => void
  clearBackup: (paneKey: string) => void

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

/** Base state object, reused by init + onCleanup reset-on-destroy. */
export const fileState: Pick<
  FileExtensionStore,
  | 'uploadingFiles'
  | 'selectedFiles'
  | 'fileOwner'
  | 'uploadOwner'
  | 'restoredFileIds'
  | 'backupByPane'
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
> = {
  uploadingFiles: new Map(),
  selectedFiles: new Map(),
  fileOwner: new Map(),
  uploadOwner: new Map(),
  restoredFileIds: new Set(),
  backupByPane: new Map(),
  messageFilesCache: new Map(),
  messageFilesLoadingSet: new Set(),
  thumbnailUrls: new Map(),
  thumbnailLoadingSet: new Set(),
  previewPageUrls: new Map(),
  previewPageLoadingSet: new Set(),
  previewPageRequested: new Map(),
  previewPageQueue: new Map(),
  previewPageErrors: new Map(),
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
}

export type FileState = typeof fileState
export type FileSet = StoreSet<FileState>
export type FileGet = () => FileExtensionStore

// Re-export the composer pane helpers so existing @/modules/file/stores/File.store
// import sites stay transparent.
export { SINGLE_PANE_KEY, composerPaneKey }
