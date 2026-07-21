import type { UpdaterGet, UpdaterSet } from '../state'

export default (set: UpdaterSet, get: UpdaterGet) => () => {
  const timer = get().pollTimer
  if (timer) {
    clearInterval(timer)
    set(s => {
      s.pollTimer = null
    })
  }
}
