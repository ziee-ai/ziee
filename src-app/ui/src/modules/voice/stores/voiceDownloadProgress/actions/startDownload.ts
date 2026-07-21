import { ApiClient } from '@/api-client'
import type { DownloadSnapshot2, DownloadVersionRequest2 } from '@/api-client/types'
import type { VoiceDownloadProgressGet, VoiceDownloadProgressSet } from '../state'
import subscribeToKeyFactory from './_subscribeToKey'
import dismissEntryFactory from './dismissEntry'

export default (set: VoiceDownloadProgressSet, get: VoiceDownloadProgressGet) => {
  const dismissEntry = dismissEntryFactory(set)
  const subscribeToKey = subscribeToKeyFactory(set, get, dismissEntry)
  return async (req: DownloadVersionRequest2): Promise<{ key: string }> => {
    const started = await ApiClient.Voice.downloadVersion(req)
    const key = started.key
    set(state => {
      const next = new Map(state.activeByKey)
      next.set(key, {
        task_id: started.task_id,
        key,
        version: started.version,
        backend: started.backend,
        status: started.status,
        bytes_received: 0,
      } as DownloadSnapshot2)
      return { activeByKey: next }
    })
    subscribeToKey(key)
    return { key }
  }
}
