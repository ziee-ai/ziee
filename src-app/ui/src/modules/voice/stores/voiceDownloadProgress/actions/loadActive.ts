import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { DownloadSnapshot2 } from '@/api-client/types'
import type { VoiceDownloadProgressGet, VoiceDownloadProgressSet } from '../state'
import subscribeToKeyFactory from './_subscribeToKey'
import dismissEntryFactory from './dismissEntry'

export default (set: VoiceDownloadProgressSet, get: VoiceDownloadProgressGet) => {
  const dismissEntry = dismissEntryFactory(set)
  const subscribeToKey = subscribeToKeyFactory(set, get, dismissEntry)
  return async (): Promise<void> => {
    // This hits the admin-only downloads endpoint; self-gate like the sibling
    // VoiceConfig/VoiceInstance stores so non-admins don't 403 on app load.
    if (!hasPermissionNow(Permissions.VoiceAdminRead)) return
    set({ loadingActive: true, error: null })
    try {
      const resp = await ApiClient.Voice.listVersionDownloads()
      const map = new Map<string, DownloadSnapshot2>()
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
