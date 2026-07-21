import type { FileGet, FileSet } from '../state'
import type { File as FileEntity } from '@/api-client/types'

/** Returns cached binary content for the file. Triggers load on first call.
 *  Returns null while loading. Only populated for binary formats (e.g. xlsx).
 *  Pass the file entity to avoid a race condition when messageFilesCache hasn't loaded yet. */
export default (_set: FileSet, get: FileGet) => (fileId: string, file?: FileEntity): ArrayBuffer | null => {
  const cached = get().fileBinaryContents.get(fileId)
  if (cached !== undefined) return cached

  if (!get().fileBinaryLoadingSet.has(fileId)) {
    Promise.resolve().then(() => get().loadFileBinaryContent(fileId, file))
  }

  return null
}
