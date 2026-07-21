import type { LlmRepositoryDrawerGet, LlmRepositoryDrawerSet } from '../state'

export default (set: LlmRepositoryDrawerSet, _get: LlmRepositoryDrawerGet) =>
  async (loading: boolean) => {
    set({ loading })
  }
