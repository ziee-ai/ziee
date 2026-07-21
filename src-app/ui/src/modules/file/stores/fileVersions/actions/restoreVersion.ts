import { ApiClient } from '@/api-client'
import loadVersionsFactory from './loadVersions'
import type { FileVersionsGet, FileVersionsSet } from '../state'
import { File } from '@/modules/file/stores/file'

export default (set: FileVersionsSet, get: FileVersionsGet) => {
  const loadVersions = loadVersionsFactory(set, get)
  return async (fileId: string, version: number): Promise<void> => {
    await ApiClient.File.restore({ file_id: fileId, version })
    await loadVersions(fileId)
    // Refresh the head entity shown in panels / cards.
    try {
      await File.loadMessageFile(fileId)
    } catch {
      /* best-effort */
    }
  }
}
