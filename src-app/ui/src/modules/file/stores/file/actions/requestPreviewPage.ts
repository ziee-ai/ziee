import type { FileGet, FileSet } from '../state'
import type { File as FileEntity } from '@/api-client/types'

/** Request a single 1-based preview page. Deduped (each page fetched once) and
 *  enqueued into a per-file queue drained sequentially (one request at a time).
 *  The viewer calls this for the visible page + the next 2. */
export default (set: FileSet, get: FileGet) => (file: FileEntity, page: number) => {
  if (page < 1 || page > file.preview_page_count) return
  if (get().previewPageRequested.get(file.id)?.has(page)) return

  set((state) => {
    const reqMap = new Map(state.previewPageRequested)
    const reqSet = new Set(reqMap.get(file.id) ?? [])
    reqSet.add(page)
    reqMap.set(file.id, reqSet)
    state.previewPageRequested = reqMap

    const qMap = new Map(state.previewPageQueue)
    qMap.set(file.id, [...(qMap.get(file.id) ?? []), page])
    state.previewPageQueue = qMap

    if (!state.previewPageUrls.has(file.id)) {
      const m = new Map(state.previewPageUrls)
      m.set(file.id, Array(file.preview_page_count).fill(null))
      state.previewPageUrls = m
    }
  })

  void get().processPreviewQueue(file)
}
