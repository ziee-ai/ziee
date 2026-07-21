import type { FileGet, FileSet } from '../state'
import { zoomStep } from '../../../viewers/image/zoom'

/** Multiplies the file's image scale by `factor`, clamped to [0.1, 8], and
 *  switches the mode to 'actual' (any non-fit zoom is an explicit scale). */
export default (set: FileSet, _get: FileGet) => (fileId: string, factor: number) => {
  set((state) => {
    const next = new Map(state.imageViewStates)
    const cur = next.get(fileId) ?? { scale: 1, mode: 'fit' as const }
    next.set(fileId, { mode: 'actual', scale: zoomStep(cur.scale, factor) })
    state.imageViewStates = next
  })
}
