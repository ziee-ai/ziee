import type { RemoteAccessSet, RemoteAccessGet } from '../state'

export default (set: RemoteAccessSet, get: RemoteAccessGet) => {
  return () => {
    const timer = get().rotationTimer
    if (timer) {
      clearInterval(timer)
      set((s) => {
        s.rotationTimer = null
      })
    }
  }
}
