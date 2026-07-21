import type { FileSet } from '../state'

/** Drop a file's tabular snapshot (e.g. on table unmount / switch to raw
 *  view) so the header's Export / Copy-selection disable rather than act on a
 *  stale, no-longer-rendered view. */
export default (set: FileSet) => (fileId: string) => {
  set((state) => {
    if (!state.fileTabularView.has(fileId)) return
    const next = new Map(state.fileTabularView)
    next.delete(fileId)
    state.fileTabularView = next
  })
}
