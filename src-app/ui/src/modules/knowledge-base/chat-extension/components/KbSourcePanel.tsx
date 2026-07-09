import { useEffect } from 'react'
import { Spinner } from '@/components/ui'
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
 * KB source viewer — opens the cited document in the chat right panel. A KB
 * document is not in the composer's file cache, so this resolves the File
 * entity by id (populating `messageFilesCache`) before rendering FilePanel.
 *
 * Task-9 scope: resolve + render the document. The page-jump + passage-highlight
 * overlay (via `File.getTextRects` and `PdfController.setPage`) is layered on in
 * the follow-up (see KB C-UI polish task).
 */
export function KbSourcePanel({ fileId }: KbSourceData) {
  const { messageFilesCache } = Stores.File
  const file = messageFilesCache.get(fileId) ?? null

  useEffect(() => {
    if (!file) void Stores.File.getFileEntityById(fileId)
  }, [fileId, file])

  if (!file) return <Spinner label="Loading document" />
  return <FilePanel file={file} />
}
