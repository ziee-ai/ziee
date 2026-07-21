import type { StoreSet } from '@ziee/framework/store-kit'

export const viewDownloadDrawerState = {
  open: false,
  loading: false,
  downloadId: null as string | null,
}

export type ViewDownloadDrawerState = typeof viewDownloadDrawerState
export type ViewDownloadDrawerSet = StoreSet<ViewDownloadDrawerState>
export type ViewDownloadDrawerGet = () => ViewDownloadDrawerState
