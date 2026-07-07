// PDF.js dynamic-import boundary (ITEM-4, DEC-11).
//
// This module statically imports `pdfjs-dist` (core + the `web/pdf_viewer`
// component + its CSS + the worker). It is itself only ever reached via a
// dynamic `import('./pdfjs')` from the viewer body, so bundlers place all of
// pdfjs-dist in a lazy async chunk — the main app bundle is unaffected until a
// PDF is actually opened.
//
// It exposes two things: `loadPdfDocument(bytes)` and `createPdfController(…)`,
// a thin wrapper around PDF.js's own `PDFViewer` + `EventBus` +
// `PDFFindController` + `PDFLinkService`. We supply only the toolbar chrome;
// the component owns continuous-scroll virtualization, incremental rendering,
// the text layer, zoom, page tracking, and find — the same component LaTeX
// Workshop's preview is built on.

import {
  getDocument,
  GlobalWorkerOptions,
  type PDFDocumentLoadingTask,
  type PDFDocumentProxy,
} from 'pdfjs-dist'
import {
  EventBus,
  PDFFindController,
  PDFLinkService,
  PDFViewer,
} from 'pdfjs-dist/web/pdf_viewer.mjs'
import 'pdfjs-dist/web/pdf_viewer.css'
// Vite serves the prebuilt worker as a static asset; setting it as workerSrc
// runs PDF.js parsing/rendering off the main thread (the smoothness win).
import workerSrc from 'pdfjs-dist/build/pdf.worker.min.mjs?url'

let workerConfigured = false
function ensureWorker(): void {
  if (workerConfigured) return
  GlobalWorkerOptions.workerSrc = workerSrc
  workerConfigured = true
}

export type ScaleValue = 'page-width' | 'page-fit' | 'page-actual'

/** Result of opening a document — keep `loadingTask` so the caller can destroy it. */
export interface LoadedPdf {
  doc: PDFDocumentProxy
  loadingTask: PDFDocumentLoadingTask
}

/**
 * Open a PDF from raw bytes. Pass a FRESH `Uint8Array` — PDF.js transfers the
 * underlying buffer to its worker (detaching it), so the caller must not reuse
 * the buffer afterward (DEC-5).
 */
export async function loadPdfDocument(data: Uint8Array): Promise<LoadedPdf> {
  ensureWorker()
  const loadingTask = getDocument({ data })
  const doc = await loadingTask.promise
  return { doc, loadingTask }
}

export interface PdfControllerCallbacks {
  /** Fired when the visible page changes (drives the "Page N of M" indicator). */
  onPageChange: (page: number) => void
  /** Fired with the current/total find-match counts. */
  onMatchesCount: (current: number, total: number) => void
  /** Fired when the numeric scale changes (drives the zoom % readout). */
  onScaleChange: (scale: number) => void
}

export interface CreatePdfControllerOptions extends PdfControllerCallbacks {
  container: HTMLDivElement
  viewer: HTMLDivElement
  doc: PDFDocumentProxy
  /** Fit mode applied once pages initialise (default `page-width`). */
  initialScaleValue?: ScaleValue
}

/** Imperative handle over a mounted `PDFViewer`, used by the toolbar. */
export interface PdfController {
  readonly pagesCount: number
  getCurrentPage(): number
  setPage(page: number): void
  setScaleValue(value: ScaleValue): void
  getScale(): number
  setScale(scale: number): void
  find(query: string): void
  findAgain(query: string, previous: boolean): void
  clearFind(): void
  destroy(): void
}

// Minimal shapes for the EventBus payloads we consume (EventBus.on is loosely
// typed as (name, Function)).
interface PageChangingEvt {
  pageNumber: number
}
interface MatchesCountEvt {
  matchesCount?: { current: number; total: number }
}

export function createPdfController(
  opts: CreatePdfControllerOptions,
): PdfController {
  const eventBus = new EventBus()
  const linkService = new PDFLinkService({ eventBus })
  const findController = new PDFFindController({ eventBus, linkService })
  const pdfViewer = new PDFViewer({
    container: opts.container,
    viewer: opts.viewer,
    eventBus,
    linkService,
    findController,
    textLayerMode: 1, // enable the selectable text layer
  })
  linkService.setViewer(pdfViewer)

  const onPagesInit = () => {
    pdfViewer.currentScaleValue = opts.initialScaleValue ?? 'page-width'
  }
  const onPageChanging = (e: PageChangingEvt) => opts.onPageChange(e.pageNumber)
  const onMatches = (e: MatchesCountEvt) =>
    opts.onMatchesCount(e.matchesCount?.current ?? 0, e.matchesCount?.total ?? 0)
  const onScaleChanging = () => opts.onScaleChange(pdfViewer.currentScale)

  eventBus.on('pagesinit', onPagesInit)
  eventBus.on('pagechanging', onPageChanging)
  eventBus.on('updatefindmatchescount', onMatches)
  eventBus.on('updatefindcontrolstate', onMatches)
  eventBus.on('scalechanging', onScaleChanging)

  pdfViewer.setDocument(opts.doc)
  linkService.setDocument(opts.doc, null)

  const dispatchFind = (query: string, again: boolean, previous: boolean) => {
    eventBus.dispatch('find', {
      source: null,
      type: again ? 'again' : '',
      query,
      caseSensitive: false,
      entireWord: false,
      highlightAll: true,
      findPrevious: previous,
      matchDiacritics: false,
    })
  }

  return {
    get pagesCount() {
      return pdfViewer.pagesCount
    },
    getCurrentPage: () => pdfViewer.currentPageNumber,
    setPage: (page: number) => {
      pdfViewer.currentPageNumber = page
    },
    setScaleValue: (value: ScaleValue) => {
      pdfViewer.currentScaleValue = value
    },
    getScale: () => pdfViewer.currentScale,
    setScale: (scale: number) => {
      pdfViewer.currentScale = scale
    },
    find: (query: string) => dispatchFind(query, false, false),
    findAgain: (query: string, previous: boolean) =>
      dispatchFind(query, true, previous),
    clearFind: () => {
      eventBus.dispatch('find', {
        source: null,
        type: '',
        query: '',
        highlightAll: false,
        findPrevious: false,
      })
    },
    destroy: () => {
      eventBus.off('pagesinit', onPagesInit)
      eventBus.off('pagechanging', onPageChanging)
      eventBus.off('updatefindmatchescount', onMatches)
      eventBus.off('updatefindcontrolstate', onMatches)
      eventBus.off('scalechanging', onScaleChanging)
    },
  }
}
