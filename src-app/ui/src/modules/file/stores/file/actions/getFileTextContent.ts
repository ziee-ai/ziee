import type { FileGet, FileSet } from '../state'
import type { File as FileEntity } from '@/api-client/types'

/** Returns cached text content for the file. Triggers load on first call.
 *  Returns null while loading. Call from render — no useEffect needed.
 *  Pass the file entity to avoid a race condition when messageFilesCache hasn't loaded yet. */
export default (_set: FileSet, get: FileGet) => (fileId: string, file?: FileEntity): string | null => {
  const cached = get().fileTextContents.get(fileId)
  if (cached !== undefined) return cached

  if (!get().fileTextLoadingSet.has(fileId)) {
    Promise.resolve().then(() => get().loadFileTextContent(fileId, file))
  }

  return null
}
