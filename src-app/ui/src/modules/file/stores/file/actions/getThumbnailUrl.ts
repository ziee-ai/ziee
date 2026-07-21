import type { FileGet, FileSet } from '../state'
import type { File as FileEntity } from '@/api-client/types'

/** Returns the cached thumbnail blob URL for a file, or null if not yet loaded.
 *  Triggers async loading when the file has has_thumbnail=true and preview_page_count>0.
 *  Components call this directly (no useEffect needed) — store handles re-renders.
 *  Pass the file entity to avoid a race condition when messageFilesCache hasn't loaded yet. */
export default (_set: FileSet, get: FileGet) => (fileId: string, fallbackFile?: FileEntity): string | null => {
  const cached = get().thumbnailUrls.get(fileId)
  if (cached) return cached

  if (!get().thumbnailLoadingSet.has(fileId)) {
    const file = get().selectedFiles.get(fileId) ?? get().messageFilesCache.get(fileId) ?? fallbackFile
    if (file?.has_thumbnail && file?.preview_page_count > 0) {
      get().loadThumbnail(fileId)
    }
  }

  return null
}
