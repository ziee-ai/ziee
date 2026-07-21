import type { FileGet, FileSet } from '../state'
import { snapshotOwned } from '../../composerOwnership'

/** Back up current files (before clearing).
 *  Snapshot ONLY the sending pane's owned entries into its own backup slot
 *  (ITEM-32) — never a whole-store snapshot, so a later restore touches only
 *  this pane and concurrent panes keep independent slots. */
export default (set: FileSet, get: FileGet) => (paneKey: string) => {
  const { selectedFiles, uploadingFiles, fileOwner, uploadOwner } = get()
  // snapshotOwned uses the SAME owner→key resolution as clearFiles, so the
  // backup captures EXACTLY the entries the paired clearFiles removes (a
  // null/undefined owner resolves to the single-pane key, not "unowned").
  const sel = snapshotOwned(selectedFiles, fileOwner, paneKey)
  const upl = snapshotOwned(uploadingFiles, uploadOwner, paneKey)
  set((state) => {
    const next = new Map(state.backupByPane)
    next.set(paneKey, { selectedFiles: sel, uploadingFiles: upl })
    state.backupByPane = next
  })
  console.log(`[FileStore] Backed up files for pane ${paneKey}`)
}
