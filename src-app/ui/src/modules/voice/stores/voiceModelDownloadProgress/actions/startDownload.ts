import { ApiClient } from '@/api-client'
import type { DownloadModelRequest, SnapshotDto } from '@/api-client/types'
import subscribeToKeyFactory from './_subscribeToKey'
import dismissEntryFactory from './dismissEntry'
import type { VoiceModelDownloadProgressGet, VoiceModelDownloadProgressSet } from '../state'

export default (set: VoiceModelDownloadProgressSet, _get: VoiceModelDownloadProgressGet) => {
  const dismissEntry = dismissEntryFactory(set)
  const subscribeToKey = subscribeToKeyFactory(set, dismissEntry)
  return async (
    req: DownloadModelRequest,
  ): Promise<{ key: string }> => {
    const started = await ApiClient.Voice.downloadModel(req)
    const key = started.key
    set(state => {
      const next = new Map(state.activeByKey)
      next.set(key, {
        task_id: started.task_id,
        key,
        name: started.name,
        status: 'downloading',
        bytes_received: 0,
      } as SnapshotDto)
      return { activeByKey: next }
    })
    subscribeToKey(key)
    return { key }
  }
}
