import type { DownloadInstance } from '@/api-client/types'
import type { LlmModelDownloadGet, LlmModelDownloadSet } from '../state'
import setupDownloadTrackingFactory from './setupDownloadTracking'

export default (set: LlmModelDownloadSet, get: LlmModelDownloadGet) => {
  const setupDownloadTracking = setupDownloadTrackingFactory(set, get)

  return async (instance: DownloadInstance): Promise<void> => {
    set((state) => ({ downloads: [...state.downloads, instance] }))
    void setupDownloadTracking()
  }
}
