import type { UpdaterGet, UpdaterSet } from '../state'

export default (set: UpdaterSet, get: UpdaterGet) => () => {
  const timer = get().dailyTimer
  if (timer) {
    clearInterval(timer)
    set(s => {
      s.dailyTimer = null
    })
  }
}
