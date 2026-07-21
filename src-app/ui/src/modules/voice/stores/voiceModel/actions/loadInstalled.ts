import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { VoiceModelGet, VoiceModelSet } from '../state'

export default (set: VoiceModelSet, _get: VoiceModelGet) =>
  async () => {
    if (!hasPermissionNow(Permissions.VoiceAdminRead)) return
    set({ loadingInstalled: true, error: null })
    try {
      const installed = await ApiClient.Voice.listModels()
      set({ installed, loadingInstalled: false })
    } catch (error) {
      set({
        error:
          error instanceof Error
            ? error.message
            : 'Failed to load installed models',
        loadingInstalled: false,
      })
    }
  }
