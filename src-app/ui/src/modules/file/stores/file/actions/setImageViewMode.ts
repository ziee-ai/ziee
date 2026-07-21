import type { FileGet, FileSet } from '../state'
import { clampScale } from '../../../viewers/image/zoom'

/** Sets the image fit-mode ('fit' | 'actual') for a file, adjusting scale:
 *  'fit' resets to scale 1, 'actual' keeps the current (or 1) scale. */
export default (set: FileSet, _get: FileGet) => (fileId: string, mode: 'fit' | 'actual') => {
  set((state) => {
    const next = new Map(state.imageViewStates)
    const cur = next.get(fileId) ?? { scale: 1, mode: 'fit' as const }
    // 'fit' pins scale back to 1 (object-contain); 'actual' keeps the
    // current scale (or 1 if it was still at fit).
    next.set(fileId, { mode, scale: mode === 'fit' ? 1 : clampScale(cur.scale) })
    state.imageViewStates = next
  })
}
