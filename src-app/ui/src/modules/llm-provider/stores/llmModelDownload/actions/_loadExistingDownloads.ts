import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { DownloadInstance } from '@/api-client/types'
import type { LlmModelDownloadGet, LlmModelDownloadSet } from '../state'

export default (set: LlmModelDownloadSet, _get: LlmModelDownloadGet) =>
  async (): Promise<void> => {
    // Permission-gate the shell-eager-load fetch: the store init fires for every
    // authenticated user; without the gate non-admins 403 on every page render.
    if (!hasPermissionNow(Permissions.LlmModelsDownloadsRead)) return
    try {
      const response = await ApiClient.LlmModel.listDownloads({ page: 1, per_page: 100 })
      // Keep only pending/downloading/failed (exclude completed and cancelled).
      const downloads = (response?.downloads ?? []).filter(
        (download: DownloadInstance) =>
          ['pending', 'downloading', 'failed'].includes(download.status),
      )
      set({ downloads })
    } catch (error) {
      console.error('Failed to load downloads:', error)
    }
  }
