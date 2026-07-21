import type { RuntimeVersionResponse } from '@/api-client/types'
import type { RuntimeDeleteConfirmGet, RuntimeDeleteConfirmSet } from '../state'

export default (set: RuntimeDeleteConfirmSet, _get: RuntimeDeleteConfirmGet) =>
  async (version: RuntimeVersionResponse | null) => {
    set({ version })
  }
