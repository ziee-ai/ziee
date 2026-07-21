import type { LlmModelDownloadSet } from '../state'

export default (set: LlmModelDownloadSet, _get: () => unknown) =>
  async (): Promise<void> => {
    set({ downloads: [] })
  }
