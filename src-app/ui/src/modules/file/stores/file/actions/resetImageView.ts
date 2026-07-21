import type { FileSet } from '../state'

/** Reset a file's image view to the default { scale: 1, mode: 'fit' }. */
export default (set: FileSet) => (fileId: string) => {
  set((state) => {
    const next = new Map(state.imageViewStates)
    next.set(fileId, { scale: 1, mode: 'fit' })
    state.imageViewStates = next
  })
}
