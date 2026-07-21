import type { StoreSet } from '@ziee/framework/store-kit'
import type { HighlightRect } from '@/api-client/types'

export interface PdfHighlightTarget {
  page: number
  rects: HighlightRect[]
}

export const pdfHighlightState = {
  targets: new Map<string, PdfHighlightTarget>(),
}

export type PdfHighlightState = typeof pdfHighlightState
export type PdfHighlightSet = StoreSet<PdfHighlightState>
export type PdfHighlightGet = () => PdfHighlightState
