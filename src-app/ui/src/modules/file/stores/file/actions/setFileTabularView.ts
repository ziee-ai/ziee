import type { FileGet, FileSet } from '../state'
import { type TabularViewState } from '../../../viewers/tabular/tableView'

/** Publishes the tabular body's current view snapshot for the header's
 *  Export / Copy-selection actions. */
export default (set: FileSet, _get: FileGet) => (fileId: string, view: TabularViewState) => {
  set((state) => {
    const next = new Map(state.fileTabularView)
    next.set(fileId, view)
    state.fileTabularView = next
  })
}
