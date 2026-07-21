import type { PdfHighlightGet, PdfHighlightSet } from '../state'
import type { PdfHighlightTarget } from '../state'

export default (set: PdfHighlightSet, _get: PdfHighlightGet) =>
  async (fileId: string, target: PdfHighlightTarget) => {
    set(draft => {
      draft.targets.set(fileId, target)
    })
  }
