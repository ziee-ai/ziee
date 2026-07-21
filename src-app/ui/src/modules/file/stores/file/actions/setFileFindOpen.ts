import type { FileSet } from '../state'

/** Opens/closes the find-in-document bar for a file. */
export default (set: FileSet) => (fileId: string, open: boolean) => {
  set((state) => {
    const next = new Map(state.fileFindOpen)
    next.set(fileId, open)
    state.fileFindOpen = next
  })
}
