import { ApiClient } from '@/api-client'
import type { UpdaterGet, UpdaterSet } from '../state'

const updaterClient = ApiClient.Updater

export default (set: UpdaterSet, _get: UpdaterGet) =>
  async () => {
    try {
      const { status } = await updaterClient.status(undefined, undefined)
      set(s => {
        s.checking = status.checking
        s.available = status.available
        s.downloading = status.downloading
        s.readyToInstall = status.ready_to_install
        s.progress = status.progress ?? null
        s.version = status.version ?? null
        s.notes = status.notes ?? null
        s.error = status.error ?? null
      })
    } catch (e) {
      set(s => {
        s.error = e instanceof Error ? e.message : 'Failed to load update status'
      })
    }
  }
