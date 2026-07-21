import type { FileGet, FileSet } from '../state'
import type { File as FileEntity } from '@/api-client/types'

/** Restore files from an existing message into the current selection.
 *  Marks them as restored so they are exempt from server deletion. */
export default (set: FileSet, get: FileGet) => (paneKey: string, files: FileEntity[]) => {
  set((state) => {
    const newSelected = new Map(state.selectedFiles)
    const newFileOwner = new Map(state.fileOwner)
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
      newFileOwner.set(file.id, paneKey)
    }
    state.selectedFiles = newSelected
    state.restoredFileIds = newRestoredIds
    state.fileOwner = newFileOwner
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
}
