import type { DownloadInstance } from '@/api-client/types'
import type { LlmModelDownloadGet } from '../state'

// Sync action — typed as returning a Promise for uniformity with the
// folder-glob pattern, but resolves immediately.
export default (_set: () => unknown, get: LlmModelDownloadGet) =>
  async (): Promise<DownloadInstance[]> => {
    return get().downloads.filter(
      (download: DownloadInstance) =>
        download.status === 'downloading' || download.status === 'pending',
    )
  }
