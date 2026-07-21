import { ApiClient } from '@/api-client'
import type { VoiceInstanceGet, VoiceInstanceSet } from '../state'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'

export default (set: VoiceInstanceSet, _get: VoiceInstanceGet) =>
  async () => {
    if (!hasPermissionNow(Permissions.VoiceAdminRead)) return
    set({ loading: true, error: null })
    try {
      const info = await ApiClient.Voice.getInstance()
      set({ info, loading: false })
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to load instance',
        loading: false,
      })
    }
  }
