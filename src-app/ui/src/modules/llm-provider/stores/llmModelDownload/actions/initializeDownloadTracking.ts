import type { LlmModelDownloadGet, LlmModelDownloadSet } from '../state'
import loadExistingDownloadsFactory from './_loadExistingDownloads'
import setupDownloadTrackingFactory from './setupDownloadTracking'

export default (set: LlmModelDownloadSet, get: LlmModelDownloadGet) => {
  const loadExistingDownloads = loadExistingDownloadsFactory(set, get)
  const setupDownloadTracking = setupDownloadTrackingFactory(set, get)

  return async (): Promise<void> => {
    if (get().isInitialized) return
    try {
      await loadExistingDownloads()
      void setupDownloadTracking()
      set({ isInitialized: true })
    } catch (error) {
      console.error('Failed to initialize download tracking:', error)
    }
  }
}
