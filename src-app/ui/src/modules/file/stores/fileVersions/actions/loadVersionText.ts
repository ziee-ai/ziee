import { ApiClient } from '@/api-client'
import type { FileVersionsGet, FileVersionsSet } from '../state'

export default (set: FileVersionsSet, _get: FileVersionsGet) =>
  async (fileId: string, version: number): Promise<void> => {
    const key = `${fileId}:${version}`
    set(s => {
      const ls = new Set(s.versionTextLoadingSet)
      ls.add(key)
      s.versionTextLoadingSet = ls
    })
    try {
      const res = await ApiClient.File.textVersion({
        file_id: fileId,
        version: String(version),
      })
      // The api-client returns a string for text/* responses (the
      // typed `Blob` is nominal); guard like File.store's text loaders
      // so calling `.text()` on a plain string doesn't throw.
      const text =
        typeof res === 'string' ? res : await (res as Blob).text()
      set(s => {
        const m = new Map(s.versionTextCache)
        m.set(key, text)
        s.versionTextCache = m
        const ls = new Set(s.versionTextLoadingSet)
        ls.delete(key)
        s.versionTextLoadingSet = ls
      })
    } catch (e) {
      set(s => {
        // Cache an error sentinel so callers don't retry on every render;
        // also distinguishes error from loading (null).
        const m = new Map(s.versionTextCache)
        m.set(key, `[error loading v${version}]`)
        s.versionTextCache = m
        const ls = new Set(s.versionTextLoadingSet)
        ls.delete(key)
        s.versionTextLoadingSet = ls
      })
      console.error('[FileVersions] failed to load version text', key, e)
    }
  }
