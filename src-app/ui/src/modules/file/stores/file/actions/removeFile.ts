import type { FileGet, FileSet } from '../state'

/** Remove a selected file. */
export default (set: FileSet, get: FileGet) => (fileId: string) => {
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
    const newFileOwner = new Map(state.fileOwner)
    newFileOwner.delete(fileId)
    state.fileOwner = newFileOwner
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
    console.log(`[FileStore] Removed non-restored file from selection: ${fileId}`)
  } else {
    console.log(`[FileStore] Removed restored file from selection (not deleted from server): ${fileId}`)
  }
}
