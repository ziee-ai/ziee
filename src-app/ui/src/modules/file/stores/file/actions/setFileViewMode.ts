import type { FileSet } from '../state'

/** Sets the view mode (compiled/raw) for a specific file tab. */
export default (set: FileSet) => (fileId: string, mode: 'compiled' | 'raw') => {
  set((state) => {
    const newModes = new Map(state.fileViewModes)
    newModes.set(fileId, mode)
    state.fileViewModes = newModes
  })
}
