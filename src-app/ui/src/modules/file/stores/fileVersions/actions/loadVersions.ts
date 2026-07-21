import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { FileVersionsGet, FileVersionsSet } from '../state'

export default (set: FileVersionsSet, _get: FileVersionsGet) =>
  async (fileId: string): Promise<void> => {
    // `sync:reconnect` fires for every store regardless of audience; skip the
    // refetch for users without `files::read` (the endpoint would 403).
    if (!hasPermissionNow(Permissions.FilesRead)) return
    set(s => {
      const ls = new Set(s.versionsLoadingSet)
      ls.add(fileId)
      s.versionsLoadingSet = ls
    })
    try {
      const versions = await ApiClient.File.listVersions({ file_id: fileId })
      set(s => {
        const m = new Map(s.versionsByFile)
        m.set(fileId, versions)
        s.versionsByFile = m
        const ls = new Set(s.versionsLoadingSet)
        ls.delete(fileId)
        s.versionsLoadingSet = ls
      })
    } catch (e) {
      set(s => {
        // Cache an empty list so a render-triggered getVersions() doesn't
        // re-attempt on every frame. A sync event re-runs loadVersions.
        if (!s.versionsByFile.has(fileId)) {
          const m = new Map(s.versionsByFile)
          m.set(fileId, [])
          s.versionsByFile = m
        }
        const ls = new Set(s.versionsLoadingSet)
        ls.delete(fileId)
        s.versionsLoadingSet = ls
      })
      console.error('[FileVersions] failed to load versions', fileId, e)
    }
  }
