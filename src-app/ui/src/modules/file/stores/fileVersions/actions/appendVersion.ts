import { ApiClient } from '@/api-client'
import { Stores } from '@ziee/framework/stores'
import loadVersionsFactory from './loadVersions'
import type { FileVersionsGet, FileVersionsSet } from '../state'

export default (set: FileVersionsSet, get: FileVersionsGet) => {
  const loadVersions = loadVersionsFactory(set, get)
  return async (fileId: string, content: string): Promise<void> => {
    await ApiClient.File.appendVersion({ file_id: fileId, content })
    await loadVersions(fileId)
    try {
      await Stores.File.loadMessageFile(fileId)
    } catch {
      /* best-effort */
    }
  }
}
