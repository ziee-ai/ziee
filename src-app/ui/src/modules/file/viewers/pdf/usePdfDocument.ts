import { useEffect, useState } from 'react'
import { ApiClient } from '@/api-client'
import type { PDFDocumentProxy } from 'pdfjs-dist'
import type * as PdfjsApi from './pdfjs'

// Doc-lifecycle hook (ITEM-4). Fetches the file's raw bytes via the
// FilesPreview-gated `/files/{id}/raw` endpoint, dynamic-imports the pdfjs
// boundary module (so pdfjs-dist stays in a lazy chunk), opens the document,
// and destroys the loading task + document on unmount. View state (zoom/page/
// find) lives in the body component, not here (DEC-6).

export type PdfDocStatus = 'loading' | 'ready' | 'error'

export interface PdfDocState {
  status: PdfDocStatus
  doc: PDFDocumentProxy | null
  /** The dynamically-imported pdfjs boundary (createPdfController, etc.). */
  api: typeof PdfjsApi | null
  error: string | null
}

export function usePdfDocument(fileId: string): PdfDocState {
  const [state, setState] = useState<PdfDocState>({
    status: 'loading',
    doc: null,
    api: null,
    error: null,
  })

  useEffect(() => {
    let cancelled = false
    let doc: PDFDocumentProxy | null = null
    let loadingTask: PdfjsApi.LoadedPdf['loadingTask'] | null = null

    setState({ status: 'loading', doc: null, api: null, error: null })

    void (async () => {
      try {
        // Blob response (binary endpoint). arrayBuffer → a FRESH Uint8Array,
        // because getDocument detaches the buffer it is handed (DEC-5).
        const blob = (await ApiClient.File.getRaw({
          file_id: fileId,
        })) as Blob
        if (cancelled) return
        const bytes = new Uint8Array(await blob.arrayBuffer())

        const api = await import('./pdfjs')
        if (cancelled) return

        const loaded = await api.loadPdfDocument(bytes)
        loadingTask = loaded.loadingTask
        doc = loaded.doc
        if (cancelled) {
          // destroying the loading task aborts network + tears down the doc + worker
          void loadingTask.destroy()
          return
        }
        setState({ status: 'ready', doc, api, error: null })
      } catch (err) {
        if (!cancelled) {
          setState({
            status: 'error',
            doc: null,
            api: null,
            error: err instanceof Error ? err.message : String(err),
          })
        }
      }
    })()

    return () => {
      cancelled = true
      // Destroying the loading task aborts in-flight work and tears down the
      // document + off-thread worker (PDFDocumentProxy has no `destroy`).
      if (loadingTask) void loadingTask.destroy()
    }
  }, [fileId])

  return state
}
