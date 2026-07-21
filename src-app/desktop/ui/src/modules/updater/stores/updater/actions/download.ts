import { ApiClient } from '@/api-client'
import type { UpdaterGet, UpdaterSet } from '../state'

const updaterClient = ApiClient.Updater

export default (set: UpdaterSet, get: UpdaterGet) =>
  async () => {
    set(s => {
      s.downloading = true
      s.progress = 0
      s.error = null
    })
    try {
      await updaterClient.download(undefined, undefined)
      get().startPolling()
    } catch (e) {
      set(s => {
        s.downloading = false
        s.error = e instanceof Error ? e.message : 'Download failed to start'
      })
    }
  }
