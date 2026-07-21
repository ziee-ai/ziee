import type { FileGet, FileSet } from '../state'
import type { File as FileEntity } from '@/api-client/types'

/** Clear a page's error + requested state and re-request it (manual retry
 *  from the viewer's error slot). */
export default (set: FileSet, get: FileGet) => (file: FileEntity, page: number) => {
  // Clear the settled error + the requested/loaded mark so requestPreviewPage
  // enqueues a fresh attempt.
  set((state) => {
    const errMap = new Map(state.previewPageErrors)
    const errSet = new Set(errMap.get(file.id) ?? [])
    errSet.delete(page)
    errMap.set(file.id, errSet)
    state.previewPageErrors = errMap

    const reqMap = new Map(state.previewPageRequested)
    const reqSet = new Set(reqMap.get(file.id) ?? [])
    reqSet.delete(page)
    reqMap.set(file.id, reqSet)
    state.previewPageRequested = reqMap
  })
  get().requestPreviewPage(file, page)
}
