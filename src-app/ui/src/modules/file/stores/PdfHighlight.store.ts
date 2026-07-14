import { enableMapSet } from 'immer'
import type { HighlightRect } from '@/api-client/types'
import { defineStore } from '@ziee/framework/store-kit'

enableMapSet()

/** A citation-highlight target: which page + which fraction-normalized rects. */
export interface PdfHighlightTarget {
  page: number
  rects: HighlightRect[]
}

/**
 * Cross-component highlight coordination for the PDF viewer. The file viewer's
 * body (`PdfJsBody`) reads its file's target reactively and drives the
 * controller's `setHighlights`; a caller that opens a document at an exact
 * passage (e.g. the KB `kb_source` panel) sets the target keyed by file id.
 *
 * This follows the file module's convention (see `types/viewer.ts`): viewer
 * bodies coordinate through a shared store, NOT threaded props.
 */
export const PdfHighlight = defineStore('PdfHighlight', {
  immer: true,
  state: {
    targets: new Map<string, PdfHighlightTarget>(),
  },
  actions: set => ({
    setTarget: (fileId: string, target: PdfHighlightTarget): void => {
      set(draft => {
        draft.targets.set(fileId, target)
      })
    },
    clearTarget: (fileId: string): void => {
      set(draft => {
        draft.targets.delete(fileId)
      })
    },
  }),
})

export const usePdfHighlightStore = PdfHighlight.store
