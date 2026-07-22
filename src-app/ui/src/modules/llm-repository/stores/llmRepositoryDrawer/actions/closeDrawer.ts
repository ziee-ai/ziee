import { setOverlayOpen } from '@/core/overlays/overlayVisibility'
import type { LlmRepositoryDrawerGet, LlmRepositoryDrawerSet } from '../state'

export default (set: LlmRepositoryDrawerSet, _get: LlmRepositoryDrawerGet) =>
  async () => {
    set({ open: false, loading: false, editingRepository: null })
    setOverlayOpen('llm-repository', false)
  }
