import { ApiClient } from '@/api-client'
import type { DownloadFromRepositoryRequest, DownloadInstance } from '@/api-client/types'
import type { LlmModelDownloadGet, LlmModelDownloadSet } from '../state'
import setupDownloadTrackingFactory from './setupDownloadTracking'

export default (set: LlmModelDownloadSet, get: LlmModelDownloadGet) => {
  const setupDownloadTracking = setupDownloadTrackingFactory(set, get)

  return async (
    request: DownloadFromRepositoryRequest,
    onStart?: (downloadId: string) => void,
  ): Promise<{ downloadId: string }> => {
    try {
      const downloadInstance: DownloadInstance = await ApiClient.LlmModel.download(request)
      set((state) => ({ downloads: [...state.downloads, downloadInstance] }))
      onStart?.(downloadInstance.id)
      void setupDownloadTracking()
      return { downloadId: downloadInstance.id }
    } catch (error) {
      console.error('Failed to initiate download:', error)
      throw error
    }
  }
}
