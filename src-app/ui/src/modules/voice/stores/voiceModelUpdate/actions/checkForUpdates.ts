import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { VoiceCatalogResponse } from '@/api-client/types'
import type { VoiceModelUpdateGet, VoiceModelUpdateSet } from '../state'

export default (set: VoiceModelUpdateSet, _get: VoiceModelUpdateGet) =>
  async (): Promise<VoiceCatalogResponse | null> => {
    if (!hasPermissionNow(Permissions.VoiceAdminRead)) return null
    set(s => {
      s.checking = true
      s.error = null
    })
    try {
      const response = await ApiClient.Voice.listModelCatalog()
      set(s => {
        s.catalog = response.models
        s.sourceReachable = response.source_reachable
        s.sourceRepo = response.source_repo
        s.hasLoaded = true
        s.checking = false
      })
      return response
    } catch (error) {
      set(s => {
        s.checking = false
        s.error =
          error instanceof Error
            ? error.message
            : 'Failed to load model catalog'
      })
      throw error
    }
  }
