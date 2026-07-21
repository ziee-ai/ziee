import { ApiClient } from '@/api-client'
import type { FileGet, FileSet } from '../state'
import type { File as FileEntity } from '@/api-client/types'

/** Internal: drains a file's page queue one request at a time. */
export default (set: FileSet, get: FileGet) => async (file: FileEntity) => {
  // One drain per file — a running drain picks up newly-enqueued pages.
  if (get().previewPageLoadingSet.has(file.id)) return
  set((state) => {
    const s = new Set(state.previewPageLoadingSet)
    s.add(file.id)
    state.previewPageLoadingSet = s
  })

  try {
    while ((get().previewPageQueue.get(file.id) ?? []).length > 0) {
      const queue = get().previewPageQueue.get(file.id) ?? []
      const page = queue[0]
      set((state) => {
        const q = new Map(state.previewPageQueue)
        q.set(file.id, queue.slice(1))
        state.previewPageQueue = q
      })

      try {
        const response = await ApiClient.File.getPreview({ file_id: file.id, page })
        const url = URL.createObjectURL(response)
        set((state) => {
          const existing =
            state.previewPageUrls.get(file.id) ??
            Array(file.preview_page_count).fill(null)
          const updated = [...existing]
          updated[page - 1] = url
          const m = new Map(state.previewPageUrls)
          m.set(file.id, updated)
          state.previewPageUrls = m
        })
      } catch (error) {
        // Record the failure so the viewer renders an explicit error/retry
        // slot instead of a spinner that never resolves (the page stays
        // `requested`, so a scroll-triggered re-request won't spin it
        // forever; `retryPreviewPage` is the deliberate re-attempt path).
        set((state) => {
          const errMap = new Map(state.previewPageErrors)
          const errSet = new Set(errMap.get(file.id) ?? [])
          errSet.add(page)
          errMap.set(file.id, errSet)
          state.previewPageErrors = errMap
        })
        console.debug(
          `[FileStore] Failed to load preview page ${page} for ${file.id}:`,
          error,
        )
      }
    }
  } finally {
    set((state) => {
      const s = new Set(state.previewPageLoadingSet)
      s.delete(file.id)
      state.previewPageLoadingSet = s
    })
  }

  // A page enqueued during the flag-teardown window would otherwise strand;
  // restart the drain if the queue refilled.
  if ((get().previewPageQueue.get(file.id) ?? []).length > 0) {
    void get().processPreviewQueue(file)
  }
}
