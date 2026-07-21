import type { FileGet, FileSet } from '../state'
import type { File as FileEntity } from '@/api-client/types'

/** Returns the cached file entity for a message file, or the fallback if not yet loaded.
 *  Triggers async loading in the background on first call for a given fileId.
 *  Components call this directly (no useEffect needed) — store handles re-renders. */
export default (_set: FileSet, get: FileGet) => (fileId: string, fallback: FileEntity): FileEntity => {
  const cached = get().messageFilesCache.get(fileId)
  if (!cached && !get().messageFilesLoadingSet.has(fileId)) {
    // Defer to avoid calling set() during React render (would cause React warning)
    Promise.resolve().then(() => get().loadMessageFile(fileId))
  }
  return cached ?? fallback
}
