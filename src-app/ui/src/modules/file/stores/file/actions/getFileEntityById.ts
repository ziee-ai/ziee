import { ApiClient } from '@/api-client'
import type { FileGet, FileSet } from '../state'

/** One-shot fetch returning the full FileEntity for a file id.
 *  Used by extension hooks (e.g. edit-conversation restore) that
 *  need a Promise rather than store-cache-backed state. Does NOT
 *  update messageFilesCache. */
export default (_set: FileSet, _get: FileGet) => async (fileId: string) => {
  return await ApiClient.File.get({ file_id: fileId })
}
