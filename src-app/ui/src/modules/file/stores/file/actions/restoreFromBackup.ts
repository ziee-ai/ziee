import type { FileGet, FileSet } from '../state'
import { mergeOwnedInto } from '../../composerOwnership'

/** Re-insert ONLY this pane's backed-up entries (owned by this pane) — a MERGE,
 *  not a wholesale replace, so a stream-error restore in one pane leaves other
 *  panes' live composer buffers untouched (ITEM-32). */
export default (set: FileSet, get: FileGet) => (paneKey: string) => {
  const backup = get().backupByPane.get(paneKey)
  if (!backup) return
  set((state) => {
    // MERGE (not replace) this pane's owned entries back in, re-stamping
    // ownership to paneKey — other panes' live entries stay untouched.
    const files = mergeOwnedInto(
      state.selectedFiles,
      state.fileOwner,
      backup.selectedFiles,
      paneKey,
    )
    state.selectedFiles = files.next
    state.fileOwner = files.nextOwner
    const uploads = mergeOwnedInto(
      state.uploadingFiles,
      state.uploadOwner,
      backup.uploadingFiles,
      paneKey,
    )
    state.uploadingFiles = uploads.next
    state.uploadOwner = uploads.nextOwner
  })
  console.log(`[FileStore] Restored files from backup for pane ${paneKey}`)
}
