import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { Stores } from '@ziee/framework/stores'
import { ApiClient } from '@/api-client'
import { fileState, type FileState } from './state'
import type { Actions } from './actions.gen'

/**
 * File store — chat-composer upload buffer + persistent file caches
 * (thumbnails, previews, content, view modes). Lives at Stores.File
 * (registered in modules/file/module.tsx). Prior name was
 * Stores.Chat.FileStore (nested via the chat-extension framework);
 * relocated out so file-domain state lives in the file module that
 * owns it.
 *
 * Uses the EAGER glob form (`{ eager: true }`): its synchronous
 * selectors (`getFileIds`, `getFiles`, `isUploading`, `getFileTextContent`,
 * `getFileBinaryContent`, `getMessageFile`, `getThumbnailUrl`,
 * `getPreviewPageUrls`, etc.) return values consumed SYNCHRONOUSLY in
 * render/handlers, so the actions must load eagerly rather than behind
 * a deferred dynamic import.
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
const FileDef = defineStore<FileState, Actions>('File', {
  immer: true,
  state: fileState,
  actions: import.meta.glob('./actions/*.ts', { eager: true }),
  init: ({ set, get, actions, onCleanup }) => {
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
      if (trackHead) void actions.loadMessageFile(fileId)
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

const FileInner = registerLazyStore(FileDef)
export const File = FileInner
// Re-export the raw Zustand store handle so gallery fixtures can use
// `store.getState()` / `store.setState()` directly.
export const { store } = FileDef
export const useFileStore = FileDef.store

// Re-export types / constants so existing `@/modules/file/stores/file`
// import sites stay transparent (they previously imported from
// `File.store`).
export {
  composerPaneKey,
  SINGLE_PANE_KEY,
  type FileUploadProgress,
  type FileState,
} from './state'
