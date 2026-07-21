import type { MemorySetupStepSet } from '../state'

export default (set: MemorySetupStepSet) =>
  async (id: string | null) => {
    set(draft => {
      draft.embeddingModelId = id
    })
  }
