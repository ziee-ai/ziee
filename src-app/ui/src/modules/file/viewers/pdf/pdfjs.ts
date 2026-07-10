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

/** A citation-highlight rectangle, fraction-normalized to the page (0..1). */
export interface HighlightRect {
  x: number
  y: number
  w: number
  h: number
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
  /**
   * Scroll to `page` and overlay `rects` (fraction-normalized, top-left origin)
   * as citation highlights. Rects are positioned in PERCENT inside PDF.js's own
   * `.page` div, so they auto-track zoom without recomputation; they're
   * re-injected on `pagerendered` (pdf.js clears page content on re-render).
   * Empty `rects` clears any existing highlight.
   */
  setHighlights(page: number, rects: HighlightRect[]): void
  clearHighlights(): void
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
interface PageRenderedEvt {
  pageNumber: number
  source?: { div?: HTMLElement }
}

const HL_LAYER_CLASS = 'ziee-citation-highlight-layer'

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

  // Citation highlight state. Rects are page-fraction (0..1); a page div in
  // pdf.js is `position: relative` and sized in px to the current scale, so
  // positioning children in PERCENT makes them track zoom for free.
  let hlPage: number | null = null
  let hlRects: HighlightRect[] = []

  const pageDivFor = (page: number): HTMLElement | null =>
    opts.viewer.querySelector<HTMLElement>(`.page[data-page-number="${page}"]`)

  const renderHighlightsInto = (pageDiv: HTMLElement) => {
    // Clear any stale layer first (idempotent across re-renders / re-injects).
    pageDiv.querySelector(`.${HL_LAYER_CLASS}`)?.remove()
    if (hlRects.length === 0) return
    const layer = document.createElement('div')
    layer.className = HL_LAYER_CLASS
    layer.style.cssText =
      'position:absolute;inset:0;pointer-events:none;z-index:2;'
    for (const r of hlRects) {
      const box = document.createElement('div')
      box.style.cssText = `position:absolute;left:${r.x * 100}%;top:${
        r.y * 100
      }%;width:${r.w * 100}%;height:${r.h * 100}%;background:color-mix(in srgb, var(--color-warning, #f59e0b) 32%, transparent);border-radius:2px;mix-blend-mode:multiply;`
      layer.appendChild(box)
    }
    pageDiv.appendChild(layer)
  }

  const renderHighlights = () => {
    if (hlPage == null) return
    const pageDiv = pageDivFor(hlPage)
    if (pageDiv) renderHighlightsInto(pageDiv)
  }

  const onPagesInit = () => {
    pdfViewer.currentScaleValue = opts.initialScaleValue ?? 'page-width'
  }
  const onPageChanging = (e: PageChangingEvt) => opts.onPageChange(e.pageNumber)
  const onMatches = (e: MatchesCountEvt) =>
    opts.onMatchesCount(e.matchesCount?.current ?? 0, e.matchesCount?.total ?? 0)
  const onScaleChanging = () => opts.onScaleChange(pdfViewer.currentScale)
  // pdf.js rebuilds a page's DOM on (re-)render (incl. zoom), wiping our layer —
  // re-inject whenever the highlighted page renders.
  const onPageRendered = (e: PageRenderedEvt) => {
    if (hlPage != null && e.pageNumber === hlPage) {
      const div = e.source?.div ?? pageDivFor(hlPage)
      if (div) renderHighlightsInto(div)
    }
  }

  eventBus.on('pagesinit', onPagesInit)
  eventBus.on('pagechanging', onPageChanging)
  eventBus.on('updatefindmatchescount', onMatches)
  eventBus.on('updatefindcontrolstate', onMatches)
  eventBus.on('scalechanging', onScaleChanging)
  eventBus.on('pagerendered', onPageRendered)

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
    setHighlights: (page: number, rects: HighlightRect[]) => {
      // Clamp to a valid page so a stale/off-by-one citation can't drive the
      // viewer out of range (pdf.js ignores an out-of-range set, but the layer
      // would then never render).
      const total = pdfViewer.pagesCount || 1
      const clamped = Math.min(Math.max(1, Math.floor(page) || 1), total)
      // Drop the old page's layer before switching target pages.
      if (hlPage != null && hlPage !== clamped) {
        pageDivFor(hlPage)?.querySelector(`.${HL_LAYER_CLASS}`)?.remove()
      }
      hlPage = clamped
      hlRects = rects
      pdfViewer.currentPageNumber = clamped
      // The target page may already be rendered (same-doc re-target); inject now.
      // If not yet rendered, `pagerendered` will inject it.
      renderHighlights()
    },
    clearHighlights: () => {
      if (hlPage != null) {
        pageDivFor(hlPage)?.querySelector(`.${HL_LAYER_CLASS}`)?.remove()
      }
      hlPage = null
      hlRects = []
    },
    destroy: () => {
      eventBus.off('pagesinit', onPagesInit)
      eventBus.off('pagechanging', onPageChanging)
      eventBus.off('updatefindmatchescount', onMatches)
      eventBus.off('updatefindcontrolstate', onMatches)
      eventBus.off('scalechanging', onScaleChanging)
      eventBus.off('pagerendered', onPageRendered)
      // Cancel in-flight page renders, then RESET the viewer: setDocument(null)
      // empties the `.pdfViewer` element (removes rendered canvases) and drops
      // pagesCount to 0, which makes any scroll/resize listener pdf.js attached
      // to the container inert. Without this, a document swap or React
      // StrictMode remount would construct a second PDFViewer over a
      // non-emptied element → stacked handlers + orphaned canvases + a leak.
      try {
        pdfViewer.cleanup()
        // setDocument's type wants a PDFDocumentProxy; passing null is the
        // documented reset path (pdf.js clears the viewer).
        pdfViewer.setDocument(null as unknown as PDFDocumentProxy)
      } catch {
        // best-effort teardown; the container is being unmounted anyway
      }
    },
  }
}
