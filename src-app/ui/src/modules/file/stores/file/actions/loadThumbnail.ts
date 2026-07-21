import { ApiClient } from '@/api-client'
import type { FileGet, FileSet } from '../state'

/** Async action: fetches page-1 preview and stores blob URL in thumbnailUrls. */
export default (set: FileSet, _get: FileGet) => async (fileId: string) => {
  set((state) => {
    const newSet = new Set(state.thumbnailLoadingSet)
    newSet.add(fileId)
    state.thumbnailLoadingSet = newSet
  })
  try {
    // Use the dedicated ~300px thumbnail (GET /files/{id}/thumbnail), not
    // the full-size preview page 1 (~2000px) — the card image only needs a
    // small image, so this is far lighter to fetch + decode.
    const response = await ApiClient.File.getThumbnail({ file_id: fileId })
    const objectUrl = URL.createObjectURL(response)
    set((state) => {
      const newUrls = new Map(state.thumbnailUrls)
      newUrls.set(fileId, objectUrl)
      const newSet = new Set(state.thumbnailLoadingSet)
      newSet.delete(fileId)
      state.thumbnailUrls = newUrls
      state.thumbnailLoadingSet = newSet
    })
  } catch (error) {
    set((state) => {
      const newSet = new Set(state.thumbnailLoadingSet)
      newSet.delete(fileId)
      state.thumbnailLoadingSet = newSet
    })
    console.debug(`[FileStore] Failed to load thumbnail for ${fileId}:`, error)
  }
}
