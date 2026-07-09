import { useEffect } from 'react'
import { Spinner } from '@/components/ui'
import { ApiClient } from '@/api-client'
import { Stores } from '@/core/stores'
import { FilePanel } from '@/modules/file/components/FilePanel'

/** Serializable payload for a `kb_source` right-panel tab. */
export interface KbSourceData {
  fileId: string
  filename: string
  page: number
  charStart: number
  charEnd: number
}

/**
 * KB source viewer — opens the cited document in the chat right panel, jumps to
 * the cited page, and highlights the exact passage.
 *
 * A KB document isn't in the composer's file cache, so this resolves the File
 * entity by id (populating `messageFilesCache`), then fetches the passage's
 * highlight rects (`File.getTextRects`, ingest-time PDF geometry) and publishes
 * them to the `PdfHighlight` store keyed by file id — the PDF viewer body reads
 * that and drives the page-jump + overlay. Non-PDF files return no rects (the
 * endpoint yields an empty set) and simply open at the top.
 */
export function KbSourcePanel({ fileId, page, charStart, charEnd }: KbSourceData) {
  const { messageFilesCache } = Stores.File
  const file = messageFilesCache.get(fileId) ?? null

  useEffect(() => {
    if (!file) void Stores.File.getFileEntityById(fileId)
  }, [fileId, file])

  // Fetch + publish the highlight target (page + fraction-normalized rects).
  // Cleared on unmount / re-target so a stale highlight never lingers on the doc.
  useEffect(() => {
    let cancelled = false
    void (async () => {
      try {
        const res = await ApiClient.File.getTextRects({
          file_id: fileId,
          page,
          start: charStart,
          end: charEnd,
        })
        if (cancelled) return
        Stores.PdfHighlight.setTarget(fileId, { page, rects: res.rects })
      } catch {
        // Fall back to a plain page jump if geometry is unavailable.
        if (!cancelled) Stores.PdfHighlight.setTarget(fileId, { page, rects: [] })
      }
    })()
    return () => {
      cancelled = true
      Stores.PdfHighlight.clearTarget(fileId)
    }
  }, [fileId, page, charStart, charEnd])

  if (!file) return <Spinner label="Loading document" />
  return <FilePanel file={file} />
}
