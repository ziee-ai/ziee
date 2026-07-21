import type { DownloadInstance } from '@/api-client/types'
import type { LlmModelDownloadGet } from '../state'

export default (_set: () => unknown, get: LlmModelDownloadGet) =>
  async (downloadId: string): Promise<DownloadInstance | undefined> => {
    return get().downloads.find((download: DownloadInstance) => download.id === downloadId)
  }
