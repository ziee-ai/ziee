import { ApiClient } from '@/api-client'
import type { UpdaterGet, UpdaterSet } from '../state'

const updaterClient = ApiClient.Updater

export default (set: UpdaterSet, get: UpdaterGet) =>
  async () => {
    set(s => {
      s.downloading = true
      s.progress = 0
      s.autoInstall = true
      s.error = null
    })
    try {
      await updaterClient.download(undefined, undefined)
      // The poll loop watches `ready_to_install` and, with autoInstall set,
      // calls install() (which restarts) the moment the bytes land.
      get().startPolling()
    } catch (e) {
      set(s => {
        s.downloading = false
        s.autoInstall = false
        s.error = e instanceof Error ? e.message : 'Update failed to start'
      })
    }
  }
