import type { RuntimeDownloadDrawerSet } from '../state'

export default (set: RuntimeDownloadDrawerSet) =>
  async () => {
    set({ open: false, engine: null })
  }
