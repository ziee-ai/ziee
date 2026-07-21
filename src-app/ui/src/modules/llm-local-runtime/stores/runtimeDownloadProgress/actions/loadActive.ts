import { ApiClient } from '@/api-client'
import subscribeToKeyFactory from '../subscribeToKey'
import type { RuntimeDownloadProgressGet, RuntimeDownloadProgressSet } from '../state'

export default (set: RuntimeDownloadProgressSet, _get: RuntimeDownloadProgressGet) => {
  const subscribeToKey = subscribeToKeyFactory(set, (key: string) => {
    set(state => {
      const next = new Map(state.activeByKey)
      next.delete(key)
      return { activeByKey: next }
    })
  })
  return async (): Promise<void> => {
    set({ loadingActive: true, error: null })
    try {
      const resp = await ApiClient.RuntimeVersion.listDownloads()
      const map = new Map<string, import('@/api-client/types').DownloadSnapshot>()
      for (const s of resp.downloads) map.set(s.key, s)
      set({ activeByKey: map, loadingActive: false })
      for (const s of resp.downloads) {
        if (s.status !== 'completed' && s.status !== 'failed') subscribeToKey(s.key)
      }
    } catch (e) {
      set({
        loadingActive: false,
        error: e instanceof Error ? e.message : 'Failed to load active downloads',
      })
    }
  }
}
