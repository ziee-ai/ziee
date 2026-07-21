import { useEffect } from 'react'
import { Spinner } from '@ziee/kit'
import { ApiClient } from '@/api-client'
import { FilePanel } from '@/modules/file/components/FilePanel'
import { useChatPaneOrNull } from '@/modules/chat/core/pane/ChatPaneContext'
import {
  FileHighlightScopeContext,
  scopedHighlightKey,
} from '@/modules/file/viewers/highlightScope'
import { PdfHighlight as PdfHighlightStore } from '@/modules/file/stores/pdfHighlight'
import { File as FileStore } from '@/modules/file/stores/file'

/** Serializable payload for a `kb_source` right-panel tab. */
export interface KbSourceData {
  fileId: string
  filename: string
  page: number
  charStart: number
  charEnd: number
  /** Passage prefix — drives find-in-document scroll for non-PDF viewers. */
  snippet?: string
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
export function KbSourcePanel({ fileId, page, charStart, charEnd, snippet }: KbSourceData) {
  const { messageFilesCache } = FileStore
  const file = messageFilesCache.get(fileId) ?? null
  // Per-pane (ITEM-49): scope the highlight/find-query keys by this pane so two
  // panes opening the SAME document's citations don't clobber each other. null on
  // the single-pane route → the bare fileId key (unchanged). The same scope is
  // provided to the viewer below so its reader resolves the same key.
  const paneScope = useChatPaneOrNull()?.paneId ?? null
  const hlKey = scopedHighlightKey(paneScope, fileId)

  useEffect(() => {
    if (!file) void FileStore.getFileEntityById(fileId)
  }, [fileId, file])

  // Non-PDF (text/markdown/code) viewers have no page geometry — drive
  // find-in-document to the passage prefix so it highlights + scrolls to it.
  useEffect(() => {
    const isPdf = file?.mime_type === 'application/pdf'
    if (file && !isPdf && snippet) FileStore.setFileFindQuery(hlKey, snippet)
  }, [file, hlKey, snippet])

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
        PdfHighlightStore.setTarget(hlKey, { page, rects: res.rects })
      } catch {
        // Fall back to a plain page jump if geometry is unavailable.
        if (!cancelled) PdfHighlightStore.setTarget(hlKey, { page, rects: [] })
      }
    })()
    return () => {
      cancelled = true
      PdfHighlightStore.clearTarget(hlKey)
    }
  }, [hlKey, page, charStart, charEnd])

  if (!file) return <Spinner label="Loading document" />
  // Provide this pane's scope so the viewer's highlight/find readers resolve the
  // same per-pane key we wrote above.
  return (
    <FileHighlightScopeContext.Provider value={paneScope}>
      <FilePanel file={file} />
    </FileHighlightScopeContext.Provider>
  )
}
