import type { PdfHighlightGet, PdfHighlightSet } from '../state'

export default (set: PdfHighlightSet, _get: PdfHighlightGet) =>
  async (fileId: string) => {
    set(draft => {
      draft.targets.delete(fileId)
    })
  }
