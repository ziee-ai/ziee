import type { FileGet, FileSet } from '../state'
import type { File as FileEntity } from '@/api-client/types'

/** Returns the cached preview page URLs for a file, or a null-filled array.
 *  PURE (no side effect): pages are loaded on demand via `requestPreviewPage`
 *  as the viewer scrolls, not all at once. */
export default (_set: FileSet, get: FileGet) => (file: FileEntity): (string | null)[] => {
  // Pure read — no auto-load. The viewer drives loading via
  // requestPreviewPage as pages scroll into view.
  return (
    get().previewPageUrls.get(file.id) ??
    Array(file.preview_page_count).fill(null)
  )
}
