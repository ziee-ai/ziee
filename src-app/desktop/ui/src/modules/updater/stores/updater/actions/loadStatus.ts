import type { UpdaterGet, UpdaterSet } from '../state'
import fetchStatusFactory from './_fetchStatus'

export default (set: UpdaterSet, get: UpdaterGet) => {
  const fetchStatus = fetchStatusFactory(set, get)
  return async () => {
    await fetchStatus()
    // Stop polling once the download settled (ready or errored).
    if (!get().downloading) get().stopPolling()
    // One-click flow: bytes ready → install + restart now.
    if (get().readyToInstall && get().autoInstall) {
      set(s => {
        s.autoInstall = false
      })
      await get().install()
    }
  }
}
