import type { UpdaterGet, UpdaterSet } from '../state'
import { DAILY_CHECK_INTERVAL_MS } from '../constants'

export default (set: UpdaterSet, get: UpdaterGet) => () => {
  if (get().dailyTimer) return // idempotent — single timer
  const timer = setInterval(() => {
    const s = get()
    // Don't disturb an in-progress download/install.
    if (s.downloading || s.readyToInstall) return
    void s.check({ resurface: true })
  }, DAILY_CHECK_INTERVAL_MS)
  set(s => {
    s.dailyTimer = timer
  })
}
