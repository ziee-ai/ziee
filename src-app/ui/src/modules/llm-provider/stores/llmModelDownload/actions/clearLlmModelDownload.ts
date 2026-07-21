import type { LlmModelDownloadSet } from '../state'

export default (set: LlmModelDownloadSet, _get: () => unknown) =>
  async (downloadId: string): Promise<void> => {
    set((state) => ({
      downloads: state.downloads.filter((download) => download.id !== downloadId),
    }))
  }
