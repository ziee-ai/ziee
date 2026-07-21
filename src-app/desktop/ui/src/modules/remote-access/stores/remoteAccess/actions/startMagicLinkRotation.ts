import { ROTATION_INTERVAL_MS } from '../state'
import type { RemoteAccessGet, RemoteAccessSet } from '../state'

export default (set: RemoteAccessSet, get: RemoteAccessGet) => () => {
  if (get().rotationTimer) return
  const timer = setInterval(() => {
    // Skip the tick when the tab is hidden — the QR isn't on screen.
    if (typeof document !== 'undefined' && document.visibilityState === 'hidden') return
    void get().rotateMagicLink()
  }, ROTATION_INTERVAL_MS)
  set(s => {
    s.rotationTimer = timer
  })
}
