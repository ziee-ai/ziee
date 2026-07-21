import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { SnapshotDto } from '@/api-client/types'
import subscribeToKeyFactory from './_subscribeToKey'
import dismissEntryFactory from './dismissEntry'
import type { VoiceModelDownloadProgressGet, VoiceModelDownloadProgressSet } from '../state'

export default (set: VoiceModelDownloadProgressSet, _get: VoiceModelDownloadProgressGet) => {
  const dismissEntry = dismissEntryFactory(set)
  const subscribeToKey = subscribeToKeyFactory(set, dismissEntry)
  return async (): Promise<void> => {
    // Admin-only downloads endpoint; self-gate like the sibling voice stores
    // so non-admins don't 403 on app load.
    if (!hasPermissionNow(Permissions.VoiceAdminRead)) return
    set({ loadingActive: true, error: null })
    try {
      const downloads = await ApiClient.Voice.listModelDownloads()
      const map = new Map<string, SnapshotDto>()
      for (const s of downloads) map.set(s.key, s)
      set({ activeByKey: map, loadingActive: false })
      for (const s of downloads) {
        if (s.status !== 'completed' && s.status !== 'failed')
          subscribeToKey(s.key)
      }
    } catch (e) {
      set({
        loadingActive: false,
        error:
          e instanceof Error
            ? e.message
            : 'Failed to load active downloads',
      })
    }
  }
}
