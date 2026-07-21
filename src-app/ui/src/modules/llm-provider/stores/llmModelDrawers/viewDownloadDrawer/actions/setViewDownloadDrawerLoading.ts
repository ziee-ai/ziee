import type { ViewDownloadDrawerSet } from '../state'

export default (
  set: ViewDownloadDrawerSet,
  _get: import('../state').ViewDownloadDrawerGet,
) => async (loading: boolean) => {
  set(s => {
    s.loading = loading
  })
}
