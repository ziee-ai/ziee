import type { LlmRepository } from '@/api-client/types'
import type { LlmRepositoryDrawerGet, LlmRepositoryDrawerSet } from '../state'

export default (set: LlmRepositoryDrawerSet, _get: LlmRepositoryDrawerGet) =>
  async (repository?: LlmRepository) => {
    set({ open: true, editingRepository: repository || null })
  }
