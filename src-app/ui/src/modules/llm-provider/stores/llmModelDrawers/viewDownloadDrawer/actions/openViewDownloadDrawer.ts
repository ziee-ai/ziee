import type { ViewDownloadDrawerSet } from '../state'

export default (
  set: ViewDownloadDrawerSet,
  _get: import('../state').ViewDownloadDrawerGet,
) => async (downloadId: string) => {
  set(s => {
    s.open = true
    s.downloadId = downloadId
  })
}
