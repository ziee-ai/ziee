import { ApiClient } from '@/api-client'
import { type AvailableUpdatesResponse2 } from '@/api-client/types'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { VoiceUpdateGet, VoiceUpdateSet } from '../state'

export default (set: VoiceUpdateSet, _get: VoiceUpdateGet) => async (): Promise<AvailableUpdatesResponse2 | null> => {
  if (!hasPermissionNow(Permissions.VoiceAdminRead)) return null
  set({ checking: true, error: null })
  try {
    const response = await ApiClient.Voice.checkVersionUpdates()
    set({ updateCheck: response, checking: false })
    return response
  } catch (error) {
    set({
      checking: false,
      error: error instanceof Error ? error.message : 'Failed to check updates',
    })
    throw error
  }
}
