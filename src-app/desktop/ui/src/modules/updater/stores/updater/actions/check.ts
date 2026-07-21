import { ApiClient } from '@/api-client'
import type { UpdaterGet, UpdaterSet } from '../state'

const updaterClient = ApiClient.Updater

export default (set: UpdaterSet, _get: UpdaterGet) =>
  async (opts?: { resurface?: boolean }) => {
    set(s => {
      s.checking = true
      s.error = null
    })
    try {
      const res = await updaterClient.check(undefined, undefined)
      set(s => {
        s.checking = false
        s.available = res.available
        s.version = res.version ?? null
        s.notes = res.notes ?? null
        // The daily background check re-surfaces a dismissed update.
        if (opts?.resurface && res.available) s.dismissed = false
      })
    } catch (e) {
      set(s => {
        s.checking = false
        s.error = e instanceof Error ? e.message : 'Update check failed'
      })
    }
  }
