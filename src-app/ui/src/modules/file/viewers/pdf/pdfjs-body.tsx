import { useEffect, useRef, useState } from 'react'
import {
  ChevronLeft,
  ChevronRight,
  Maximize,
  MoveHorizontal,
  Scan,
  Search,
  TriangleAlert,
  X,
  ZoomIn,
  ZoomOut,
} from 'lucide-react'
import { Button, Input, Spin, Text, Tooltip } from '@/components/ui'
import type { FileViewerSlotProps } from '../../types/viewer'
import type { PdfController, ScaleValue } from './pdfjs'
import { usePdfDocument } from './usePdfDocument'
import { clampPage, parseJump } from './nav'
import { nextZoomStep } from './zoom'

// Client-side PDF viewer (ITEM-5/6/7/8/9, DEC-11). Mounts PDF.js's own
// `PDFViewer` component — native continuous-scroll virtualization, incremental
// rendering, and a selectable text layer — and drives it from a co-located
// toolbar (page nav / zoom / find). The toolbar lives IN the body (not the
// header slot) because header and body are independent components and the
// controller is component-local view state (DEC-6). Used only for the
// `application/pdf` entry; office docs keep the legacy image body.
export function PdfJsBody(props: FileViewerSlotProps) {
  // The PDF entry declares no `inline:`, so only `{file}` ever reaches here.
  if (!('file' in props)) return null
  const { file } = props
  const { status, doc, api, error } = usePdfDocument(file.id)

  const containerRef = useRef<HTMLDivElement>(null)
  const viewerRef = useRef<HTMLDivElement>(null)
  const controllerRef = useRef<PdfController | null>(null)

  const [numPages, setNumPages] = useState(0)
  const [currentPage, setCurrentPage] = useState(1)
  const [pageInput, setPageInput] = useState('1')

  const [findOpen, setFindOpen] = useState(false)
  const [findQuery, setFindQuery] = useState('')
  const [matches, setMatches] = useState({ current: 0, total: 0 })
  const findInputRef = useRef<HTMLInputElement>(null)

  // Instantiate the PDFViewer controller once the doc + DOM refs are ready.
  useEffect(() => {
    if (status !== 'ready' || !doc || !api) return
    const container = containerRef.current
    const viewer = viewerRef.current
    if (!container || !viewer) return

    const controller = api.createPdfController({
      container,
      viewer,
      doc,
      onPageChange: (p) => {
        setCurrentPage(p)
        setPageInput(String(p))
      },
      onMatchesCount: (current, total) => setMatches({ current, total }),
      onScaleChange: () => {
        /* numeric scale drives no readout today; fit modes are the affordance */
      },
    })
    controllerRef.current = controller
    setNumPages(doc.numPages)

    return () => {
      controller.destroy()
      controllerRef.current = null
    }
  }, [status, doc, api])

  const goToPage = (p: number) => controllerRef.current?.setPage(clampPage(p, numPages))
  const stepPage = (delta: number) => goToPage(currentPage + delta)
  const zoom = (dir: 1 | -1) => {
    const c = controllerRef.current
    if (!c) return
    c.setScale(nextZoomStep(c.getScale(), dir))
  }
  const setFit = (value: ScaleValue) => controllerRef.current?.setScaleValue(value)

  const runFind = (q: string) => {
    setFindQuery(q)
    const c = controllerRef.current
    if (!c) return
    if (q) c.find(q)
    else {
      c.clearFind()
      setMatches({ current: 0, total: 0 })
    }
  }
  const findStep = (previous: boolean) => {
    if (findQuery) controllerRef.current?.findAgain(findQuery, previous)
  }

  const openFind = () => {
    setFindOpen(true)
    // focus after the input mounts
    requestAnimationFrame(() => findInputRef.current?.focus())
  }
  const closeFind = () => {
    setFindOpen(false)
    runFind('')
  }

  const onKeyDown = (e: React.KeyboardEvent) => {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'f') {
      e.preventDefault()
      openFind()
    } else if (e.key === 'Escape' && findOpen) {
      closeFind()
    }
  }

  const ready = status === 'ready'

  return (
    <div className="flex h-full flex-col" onKeyDown={onKeyDown}>
      {/* Toolbar */}
      <div
        className="flex flex-none items-center gap-1 border-b px-2 py-1"
        data-testid="file-pdf-toolbar"
      >
        <Tooltip title="Previous page">
          <Button
            size="icon"
            variant="ghost"
            aria-label="Previous page"
            disabled={!ready || currentPage <= 1}
            onClick={() => stepPage(-1)}
            data-testid="file-pdf-prev-page"
          >
            <ChevronLeft />
          </Button>
        </Tooltip>
        <div className="flex items-center gap-1">
          <Input
            aria-label="Page number"
            className="h-7 w-12 text-center"
            value={pageInput}
            disabled={!ready}
            onChange={(e) => setPageInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') {
                const target = parseJump(pageInput, numPages)
                if (target != null) goToPage(target)
              }
            }}
            onBlur={() => {
              const target = parseJump(pageInput, numPages)
              if (target != null) goToPage(target)
              else setPageInput(String(currentPage))
            }}
            data-testid="file-pdf-page-input"
          />
          <Text type="secondary" className="!text-xs whitespace-nowrap" data-testid="file-pdf-page-indicator">
            of {numPages || '–'}
          </Text>
        </div>
        <Tooltip title="Next page">
          <Button
            size="icon"
            variant="ghost"
            aria-label="Next page"
            disabled={!ready || currentPage >= numPages}
            onClick={() => stepPage(1)}
            data-testid="file-pdf-next-page"
          >
            <ChevronRight />
          </Button>
        </Tooltip>

        <div className="mx-1 h-5 w-px bg-border" />

        <Tooltip title="Zoom out">
          <Button size="icon" variant="ghost" aria-label="Zoom out" disabled={!ready} onClick={() => zoom(-1)} data-testid="file-pdf-zoom-out">
            <ZoomOut />
          </Button>
        </Tooltip>
        <Tooltip title="Zoom in">
          <Button size="icon" variant="ghost" aria-label="Zoom in" disabled={!ready} onClick={() => zoom(1)} data-testid="file-pdf-zoom-in">
            <ZoomIn />
          </Button>
        </Tooltip>
        <Tooltip title="Fit width">
          <Button size="icon" variant="ghost" aria-label="Fit width" disabled={!ready} onClick={() => setFit('page-width')} data-testid="file-pdf-fit-width">
            <MoveHorizontal />
          </Button>
        </Tooltip>
        <Tooltip title="Fit page">
          <Button size="icon" variant="ghost" aria-label="Fit page" disabled={!ready} onClick={() => setFit('page-fit')} data-testid="file-pdf-fit-page">
            <Maximize />
          </Button>
        </Tooltip>
        <Tooltip title="Actual size">
          <Button size="icon" variant="ghost" aria-label="Actual size" disabled={!ready} onClick={() => setFit('page-actual')} data-testid="file-pdf-actual-size">
            <Scan />
          </Button>
        </Tooltip>

        <div className="mx-1 h-5 w-px bg-border" />

        <Tooltip title="Find (Ctrl+F)">
          <Button size="icon" variant="ghost" aria-label="Find in document" disabled={!ready} onClick={openFind} data-testid="file-pdf-find-toggle">
            <Search />
          </Button>
        </Tooltip>
      </div>

      {/* Find bar */}
      {findOpen && (
        <div className="flex flex-none items-center gap-1 border-b px-2 py-1" data-testid="file-pdf-find-bar">
          <Input
            ref={findInputRef}
            aria-label="Find in document"
            placeholder="Find in document"
            className="h-7 flex-1"
            value={findQuery}
            onChange={(e) => runFind(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') findStep(e.shiftKey)
            }}
            data-testid="file-pdf-find-input"
          />
          <Text type="secondary" className="!text-xs whitespace-nowrap" data-testid="file-pdf-find-count">
            {findQuery ? `${matches.current} of ${matches.total}` : ''}
          </Text>
          <Button size="icon" variant="ghost" aria-label="Previous match" disabled={!matches.total} onClick={() => findStep(true)} data-testid="file-pdf-find-prev">
            <ChevronLeft />
          </Button>
          <Button size="icon" variant="ghost" aria-label="Next match" disabled={!matches.total} onClick={() => findStep(false)} data-testid="file-pdf-find-next">
            <ChevronRight />
          </Button>
          <Button size="icon" variant="ghost" aria-label="Close find" onClick={closeFind} data-testid="file-pdf-find-close">
            <X />
          </Button>
        </div>
      )}

      {/* Render surface — the PDFViewer manages its own scrolling + virtualization. */}
      <div className="relative min-h-0 flex-1">
        <div ref={containerRef} className="absolute inset-0 overflow-auto" data-testid="file-pdf-container">
          <div ref={viewerRef} className="pdfViewer" />
        </div>

        {status === 'loading' && (
          <div className="absolute inset-0 flex items-center justify-center" data-testid="file-pdf-loading">
            <Spin label="Loading PDF" />
          </div>
        )}
        {status === 'error' && (
          <div
            className="absolute inset-0 flex flex-col items-center justify-center gap-2 p-6 text-center"
            data-testid="file-pdf-error"
          >
            <TriangleAlert className="size-8 text-warning" />
            <Text type="secondary" className="text-sm">
              Couldn't display this PDF.
            </Text>
            {error && (
              <Text type="secondary" className="!text-xs">
                {error}
              </Text>
            )}
          </div>
        )}
      </div>
    </div>
  )
}
