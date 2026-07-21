import { ApiClient } from '@/api-client'
import type { UpdaterGet, UpdaterSet } from '../state'

const updaterClient = ApiClient.Updater

export default (set: UpdaterSet, _get: UpdaterGet) =>
  async () => {
    set(s => {
      s.error = null
    })
    try {
      // The backend quits + restarts on success, so no meaningful response.
      await updaterClient.install(undefined, undefined)
    } catch (e) {
      set(s => {
        s.error = e instanceof Error ? e.message : 'Install failed'
      })
    }
  }
