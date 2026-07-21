import { ApiClient } from '@/api-client'
import type { VoiceInstanceGet, VoiceInstanceSet } from '../state'

export default (set: VoiceInstanceSet, _get: VoiceInstanceGet) =>
  async () => {
    set({ busy: true, error: null })
    try {
      const info = await ApiClient.Voice.stopInstance()
      set({ info, busy: false })
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to stop instance',
        busy: false,
      })
      throw error
    }
  }
