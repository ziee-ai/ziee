import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { pdfHighlightState, type PdfHighlightState } from './state'
import type { Actions } from './actions.gen'

const PdfHighlightDef = defineStore<PdfHighlightState, Actions>('PdfHighlight', {
  immer: true,
  state: pdfHighlightState,
  actions: import.meta.glob('./actions/*.ts'),
})
export const PdfHighlight = registerLazyStore(PdfHighlightDef)
export const usePdfHighlightStore = PdfHighlightDef.store
