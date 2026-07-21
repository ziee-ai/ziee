import type { FileGet, FileSet } from '../state'
import { ownedIds } from '../../composerOwnership'

/** Clear ONE pane's composer buffer (ITEM-32) — called after that pane sends a
 *  message or cancels an edit. Only that pane's files are removed, so a split
 *  pane's attachments never clear the other pane's. Session (non-restored)
 *  files have their thumbnail/preview blob URLs revoked; restored files keep
 *  theirs (shared with the message-display cache). */
export default (set: FileSet, get: FileGet) => (paneKey: string) => {
  console.log('[FileStore] clearFiles() called for pane', paneKey)
  const restoredIds = get().restoredFileIds
  const fileOwner = get().fileOwner
  const uploadOwner = get().uploadOwner
  const paneFileIds = ownedIds(get().selectedFiles.keys(), fileOwner, paneKey)
  const paneUploadIds = ownedIds(
    get().uploadingFiles.keys(),
    uploadOwner,
    paneKey,
  )
  const sessionFileIds = paneFileIds.filter(id => !restoredIds.has(id))
  if (sessionFileIds.length > 0) {
    console.log('[FileStore] Session files cleared (server deletion if applicable):', sessionFileIds)
  }
  // Revoke + evict thumbnail/preview blob URLs ONLY for this pane's
  // session-uploaded files.
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
    const newSelected = new Map(state.selectedFiles)
    for (const id of paneFileIds) newSelected.delete(id)
    const newUploading = new Map(state.uploadingFiles)
    for (const id of paneUploadIds) newUploading.delete(id)
    const newFileOwner = new Map(state.fileOwner)
    for (const id of paneFileIds) newFileOwner.delete(id)
    const newUploadOwner = new Map(state.uploadOwner)
    for (const id of paneUploadIds) newUploadOwner.delete(id)
    const newRestored = new Set(state.restoredFileIds)
    for (const id of paneFileIds) newRestored.delete(id)
    state.selectedFiles = newSelected
    state.uploadingFiles = newUploading
    state.fileOwner = newFileOwner
    state.uploadOwner = newUploadOwner
    state.restoredFileIds = newRestored
    state.thumbnailUrls = newThumbnails
    state.thumbnailLoadingSet = newLoadingSet
    state.previewPageUrls = newPageUrls
    state.previewPageLoadingSet = newPageLoadingSet
  })
}
