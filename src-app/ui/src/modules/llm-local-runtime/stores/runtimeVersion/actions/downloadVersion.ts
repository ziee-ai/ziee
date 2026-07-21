import type { DownloadVersionRequest } from '@/api-client/types'
import type { RuntimeVersionGet, RuntimeVersionSet } from '../state'
import { RuntimeDownloadProgress } from '@/modules/llm-local-runtime/stores/runtimeDownloadProgress'

export default (set: RuntimeVersionSet, _get: RuntimeVersionGet) =>
  async (request: DownloadVersionRequest): Promise<{ key: string }> => {
    set(s => {
      s.error = null
    })
    try {
      return await RuntimeDownloadProgress.startDownload(request)
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Download failed'
      set(s => {
        s.error = message
      })
      throw error
    }
  }
