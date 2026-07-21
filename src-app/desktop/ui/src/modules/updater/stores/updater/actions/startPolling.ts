import type { UpdaterGet, UpdaterSet } from '../state'
import { POLL_INTERVAL_MS } from '../constants'

export default (set: UpdaterSet, get: UpdaterGet) => () => {
  if (get().pollTimer) return // idempotent — single timer
  const timer = setInterval(() => {
    void get().loadStatus()
  }, POLL_INTERVAL_MS)
  set(s => {
    s.pollTimer = timer
  })
}
