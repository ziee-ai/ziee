import type { ViewDownloadDrawerSet } from '../state'

export default (
  set: ViewDownloadDrawerSet,
  _get: import('../state').ViewDownloadDrawerGet,
) => async () => {
  set(s => {
    s.open = false
    s.loading = false
    s.downloadId = null
  })
}
