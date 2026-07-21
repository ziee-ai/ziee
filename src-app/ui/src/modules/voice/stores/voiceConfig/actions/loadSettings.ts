import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { VoiceConfigGet, VoiceConfigSet } from '../state'

export default (set: VoiceConfigSet, _get: VoiceConfigGet) =>
  async () => {
    if (!hasPermissionNow(Permissions.VoiceAdminRead)) return
    set({ loadingSettings: true, error: null })
    try {
      const settings = await ApiClient.Voice.getSettings()
      set({ settings, loadingSettings: false })
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to load voice settings',
        loadingSettings: false,
      })
    }
  }
