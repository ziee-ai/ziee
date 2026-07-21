import { ApiClient } from '@/api-client'
import type { FileGet } from '../state'

/** Opens the file in a new browser tab. Mints a fresh short-lived download
 *  token (so the unauthenticated tab navigation still succeeds — a plain
 *  `<a target=_blank>` can't send the bearer header) and opens the
 *  same-origin `download-with-token` URL. Throws on failure. */
export default (_set: never, _get: FileGet) => async (fileId: string) => {
  // Mint a fresh short-lived token: a new-tab navigation can't carry the
  // bearer header, but the download-with-token endpoint authenticates via
  // the query param. Same-origin relative URL so it works in dev + prod.
  const { token } = await ApiClient.File.generateDownloadToken({ file_id: fileId })
  const url = `/api/files/${fileId}/download-with-token?token=${encodeURIComponent(token)}`
  window.open(url, '_blank', 'noopener,noreferrer')
}
