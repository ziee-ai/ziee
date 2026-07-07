import { useEffect, useRef } from 'react'
import { File, TriangleAlert } from 'lucide-react'
import type { OverlayScrollbarsComponentRef } from 'overlayscrollbars-react'
import { Alert, Button, ScrollArea, Spin, Text } from '@/components/ui'
import { Stores } from '@/core/stores'
import type { FileViewerSlotProps } from '../../types/viewer'

// NOTE: since the PDF.js viewer (`pdfjs-body.tsx`) took over the
// `application/pdf` entry, `PdfBody` is the **office/image** path — it renders
// the backend's pre-rasterized preview images for DOCX/DOC/RTF/ODT (which are
// converted to PDF→images server-side and have no client-side PDF to feed
// PDF.js). Real PDFs no longer reach this component.
export function PdfBody(props: FileViewerSlotProps) {
  // PDF viewer is not inline-capable (its module declares no `inline:`).
  // The chat dispatcher won't call this in source context, but guard for
  // type-narrowing safety.
  if (!('file' in props)) return null
  const { file } = props

  // Subscribe to previewPageUrls Map directly so we re-render as each
  // page slot loads. Calling the `getPreviewPageUrls()` action instead
  // would only subscribe to the function reference (whose identity never
  // changes), so the body would freeze at the initial placeholder array.
  const previewPageUrls = Stores.File.previewPageUrls
  const cachedUrls = previewPageUrls.get(file.id)
  const pageUrls = cachedUrls ?? Stores.File.getPreviewPageUrls(file)
  // Subscribe to per-page errors so a settled failure renders an error/retry
  // slot rather than a spinner that never resolves (finding #17).
  const pageErrors = Stores.File.previewPageErrors.get(file.id)

  // Total page count of the source document, when the backend was able
  // to compute it (PDF / DOCX-via-PDF). May be undefined for
  // never-paged formats or for legacy rows uploaded before
  // `processing_metadata.page_count` was added — in that case we
  // assume `preview_page_count` is the true total (no truncation
  // banner). `processing_metadata` is typed as `any` in the OpenAPI
  // surface, hence the cast + the explicit null/undefined check.
  const meta = (file.processing_metadata ?? {}) as { page_count?: number }
  const totalPages =
    typeof meta.page_count === 'number' ? meta.page_count : undefined
  const truncated =
    typeof totalPages === 'number' && totalPages > file.preview_page_count

  // Load pages on demand: as a page slot scrolls into view (with a prefetch
  // margin), request that page + the next 2. The store dedupes and fetches
  // sequentially, so pages load one-by-one, only around the viewport.
  // ScrollArea is OverlayScrollbars, so the element that actually scrolls (and
  // must be the IntersectionObserver root) is the OS *viewport*, not the host.
  const osRef = useRef<OverlayScrollbarsComponentRef>(null)
  useEffect(() => {
    const root = osRef.current?.osInstance()?.elements().viewport
    if (!root || file.preview_page_count === 0) return
    // Always load the first page(s) up front. Until page 1 renders, every empty
    // slot is short, so relying only on visibility would flag them all as
    // visible and load everything — the reserved placeholder height (below)
    // plus this eager first request keep the window small.
    Stores.File.requestPreviewPage(file, 1)
    Stores.File.requestPreviewPage(file, 2)
    Stores.File.requestPreviewPage(file, 3)
    const io = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (!entry.isIntersecting) continue
          const idx = Number((entry.target as HTMLElement).dataset.pageIndex)
          if (Number.isNaN(idx)) continue
          // visible page (idx+1) + the next 2
          Stores.File.requestPreviewPage(file, idx + 1)
          Stores.File.requestPreviewPage(file, idx + 2)
          Stores.File.requestPreviewPage(file, idx + 3)
        }
      },
      { root, rootMargin: '200px 0px' },
    )
    root.querySelectorAll('[data-page-index]').forEach((el) => io.observe(el))
    return () => io.disconnect()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [file.id, file.preview_page_count])

  if (file.preview_page_count === 0) {
    return (
      <div className="flex flex-col items-center gap-2 py-8">
        <File style={{ fontSize: 48 }} />
        <Text type="secondary">Preview not available for this file</Text>
      </div>
    )
  }

  return (
    <ScrollArea ref={osRef} axis="both" className="h-full">
    <div className="flex flex-col gap-6 p-4">
      {truncated && (
        <Alert
          tone="info"
          title={
            `Showing first ${file.preview_page_count} of ${totalPages} pages. ` +
            `Download the file to view all pages.`
          }
          className="flex-shrink-0"
          data-testid="file-pdf-truncated-alert"
        />
      )}
      {pageUrls.map((url, i) => (
        <div
          key={i}
          data-page-index={i}
          className="flex flex-col items-center gap-1"
          // Browser-native virtualization: skip painting / image
          // decode for pages that are offscreen. Each page is
          // reserved as ~800px (rough A4-at-display height) so the
          // scrollbar geometry stays accurate without measuring
          // every page. Cuts initial render cost for 50+-page PDFs
          // from all-pages-decoded to viewport-worth-of-pages.
          // `loading="lazy"` on the img is a defense-in-depth — even
          // when content-visibility doesn't apply (older browsers),
          // the image network fetch + decode is deferred.
          style={{
            contentVisibility: 'auto',
            containIntrinsicSize: 'auto 800px',
          }}
        >
          <Text type="secondary" className="!text-xs">
            Page {i + 1} of {totalPages ?? file.preview_page_count}
          </Text>
          {url ? (
            <img
              src={url}
              alt={`Page ${i + 1}`}
              className="w-full object-contain rounded shadow"
              loading="lazy"
            />
          ) : pageErrors?.has(i + 1) ? (
            // Settled failure — an explicit, actionable error slot (NOT an
            // endless spinner). A settled error hugs its content (a modest
            // floor centers the card) instead of reserving a full page-height
            // band of dead space; the transient loading slot below still
            // reserves the page height to avoid a scroll jump while the image
            // decodes. Retry re-expands the slot to the loaded page.
            <div
              className="w-full flex flex-col items-center justify-center gap-2 min-h-[160px] p-6 text-center"
              data-testid={`file-pdf-page-error-${i + 1}`}
            >
              <TriangleAlert className="size-8 text-warning" />
              <Text type="secondary" className="text-xs">
                Couldn't load page {i + 1}.
              </Text>
              <Button
                size="default"
                variant="outline"
                onClick={() => Stores.File.retryPreviewPage(file, i + 1)}
                data-testid={`file-pdf-page-retry-${i + 1}`}
              >
                Retry
              </Button>
            </div>
          ) : (
            // Reserve a page-sized height so unloaded pages aren't tiny —
            // otherwise every slot would fit the viewport at once and the
            // visibility check would request all pages.
            <div className="w-full flex items-center justify-center min-h-[800px]">
              <Spin label="Loading" />
            </div>
          )}
        </div>
      ))}
    </div>
    </ScrollArea>
  )
}
