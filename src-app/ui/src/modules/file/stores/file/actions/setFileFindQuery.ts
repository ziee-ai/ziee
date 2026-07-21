import type { FileGet, FileSet } from '../state'

/** Set the external find query for a file (opens find + scrolls to it). */
export default (set: FileSet, _get: FileGet) => (fileId: string, query: string) => {
  set((state) => {
    const nq = new Map(state.fileFindQuery)
    nq.set(fileId, query)
    state.fileFindQuery = nq
    const no = new Map(state.fileFindOpen)
    no.set(fileId, true)
    state.fileFindOpen = no
  })
}
