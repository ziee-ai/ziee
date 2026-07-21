import type { RuntimeDownloadDrawerSet } from '../state'
import type { RuntimeEngine } from '../../../types'

export default (set: RuntimeDownloadDrawerSet) =>
  async (engine: RuntimeEngine) => {
    set({ open: true, engine })
  }
