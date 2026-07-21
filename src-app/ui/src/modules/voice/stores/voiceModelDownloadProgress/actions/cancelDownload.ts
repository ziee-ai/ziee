import { ApiClient } from '@/api-client'
import subscribeToKeyFactory from './_subscribeToKey'
import dismissEntryFactory from './dismissEntry'
import type { VoiceModelDownloadProgressGet, VoiceModelDownloadProgressSet } from '../state'

export default (set: VoiceModelDownloadProgressSet, _get: VoiceModelDownloadProgressGet) => {
  const dismissEntry = dismissEntryFactory(set)
  const subscribeToKey = subscribeToKeyFactory(set, dismissEntry)
  return async (key: string): Promise<void> => {
    try {
      await ApiClient.Voice.cancelModelDownload({ key })
    } finally {
      subscribeToKey.abort(key)
    }
  }
}
