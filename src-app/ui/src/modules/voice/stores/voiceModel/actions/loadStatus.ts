import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { VoiceModelGet, VoiceModelSet } from '../state'

export default (set: VoiceModelSet, _get: VoiceModelGet) =>
  async () => {
    if (!hasPermissionNow(Permissions.VoiceAdminRead)) return
    set({ loading: true, error: null })
    try {
      const status = await ApiClient.Voice.getModelStatus()
      set({ status, loading: false })
    } catch (error) {
      set({
        error:
          error instanceof Error
            ? error.message
            : 'Failed to load model status',
        loading: false,
      })
    }
  }
