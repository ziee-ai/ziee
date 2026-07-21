import { ApiClient } from '@/api-client'
import type { LlmModelDownloadSet } from '../state'

export default (set: LlmModelDownloadSet, _get: () => unknown) =>
  async (downloadId: string): Promise<void> => {
    try {
      await ApiClient.LlmModel.deleteDownload({ download_id: downloadId })
      set((state) => ({
        downloads: state.downloads.filter((download) => download.id !== downloadId),
      }))
    } catch (error) {
      console.error('Failed to delete download:', error)
      throw error
    }
  }
