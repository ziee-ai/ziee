import type { FileSet } from '../state'

/** Turns word-wrap on/off for a file's raw/code view. */
export default (set: FileSet) => (fileId: string, on: boolean) => {
  set((state) => {
    const next = new Map(state.fileWordWrap)
    next.set(fileId, on)
    state.fileWordWrap = next
  })
}
