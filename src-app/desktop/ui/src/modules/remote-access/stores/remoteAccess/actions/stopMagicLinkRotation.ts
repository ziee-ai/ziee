import type { RemoteAccessGet, RemoteAccessSet } from '../state'

export default (set: RemoteAccessSet, get: RemoteAccessGet) => () => {
  const timer = get().rotationTimer
  if (timer) {
    clearInterval(timer)
    set(s => {
      s.rotationTimer = null
    })
  }
}
