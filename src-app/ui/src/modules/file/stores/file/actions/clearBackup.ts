import type { FileSet, FileGet } from '../state'

/** Drop this pane's backup slot. */
export default (set: FileSet, _get: FileGet) => (paneKey: string) => {
  set((state) => {
    const next = new Map(state.backupByPane)
    next.delete(paneKey)
    state.backupByPane = next
  })
  console.log('[FileStore] Cleared file backup for pane', paneKey)
}
