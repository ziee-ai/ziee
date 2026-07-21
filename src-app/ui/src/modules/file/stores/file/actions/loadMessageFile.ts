import { ApiClient } from '@/api-client'
import type { FileGet, FileSet } from '../state'

/** Async action: fetches full file entity and updates messageFilesCache. */
export default (set: FileSet, _get: FileGet) => async (fileId: string) => {
  set((state) => {
    const newSet = new Set(state.messageFilesLoadingSet)
    newSet.add(fileId)
    state.messageFilesLoadingSet = newSet
  })
  try {
    const fileInfo = await ApiClient.File.get({ file_id: fileId })
    set((state) => {
      const newCache = new Map(state.messageFilesCache)
      // Only cache a well-formed entity. A malformed response (missing id
      // — e.g. a transient backend hiccup) must not poison the cache over
      // the caller-supplied fallback (the content-block-derived entity that
      // already carries filename + size); caching it would surface blanks /
      // "NaN" size in its place.
      if (fileInfo && (fileInfo as { id?: string }).id) {
        newCache.set(fileId, fileInfo)
      }
      const newSet = new Set(state.messageFilesLoadingSet)
      newSet.delete(fileId)
      state.messageFilesCache = newCache
      state.messageFilesLoadingSet = newSet
    })
    // NOTE: thumbnails are intentionally NOT eager-loaded here. The
    // consumers that actually display one (FileCard, ImageBody) call
    // getThumbnailUrl() when they render. This lets viewport-gated inline
    // previews avoid fetching/decoding thumbnails for off-screen files on
    // reload — the fix for laggy reloads with many inline images.
  } catch (error) {
    set((state) => {
      const newSet = new Set(state.messageFilesLoadingSet)
      newSet.delete(fileId)
      state.messageFilesLoadingSet = newSet
    })
    console.error(`[FileStore] Failed to load message file ${fileId}:`, error)
  }
}
