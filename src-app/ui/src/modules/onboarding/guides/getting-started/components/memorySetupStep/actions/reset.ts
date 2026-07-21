import type { MemorySetupStepSet } from '../state'

export default (set: MemorySetupStepSet) =>
  async () => {
    set(draft => {
      draft.enableMemory = false
      draft.embeddingModelId = null
      draft.availableModels = []
      draft.loading = false
      draft.saving = false
      draft.error = null
    })
  }
