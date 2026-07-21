import type { RemoteAccessSet, RemoteAccessGet } from '../state'

// Rotation interval: 4 min, comfortably under the 5-min server-side TTL.
const ROTATION_INTERVAL_MS = 4 * 60 * 1000

export default (set: RemoteAccessSet, get: RemoteAccessGet) => {
  return () => {
    if (get().rotationTimer) return
    const timer = setInterval(() => {
      // Skip the tick when the tab is hidden — the QR isn't on screen.
      if (typeof document !== 'undefined' && document.visibilityState === 'hidden') return
      void get().rotateMagicLink()
    }, ROTATION_INTERVAL_MS)
    set((s) => {
      s.rotationTimer = timer
    })
  }
}
