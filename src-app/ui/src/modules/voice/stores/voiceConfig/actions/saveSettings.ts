import { ApiClient } from '@/api-client'
import type { UpdateVoiceSettingsRequest } from '@/api-client/types'
import type { VoiceConfigGet, VoiceConfigSet } from '../state'

export default (set: VoiceConfigSet, _get: VoiceConfigGet) =>
  async (req: UpdateVoiceSettingsRequest) => {
    set({ savingSettings: true, error: null })
    try {
      const settings = await ApiClient.Voice.updateSettings(req)
      set({ settings, savingSettings: false })
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to save voice settings',
        savingSettings: false,
      })
      throw error
    }
  }
