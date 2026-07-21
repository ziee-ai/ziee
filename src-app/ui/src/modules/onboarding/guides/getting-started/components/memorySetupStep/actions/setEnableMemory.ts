import type { MemorySetupStepSet } from '../state'

export default (set: MemorySetupStepSet) =>
  async (enabled: boolean) => {
    set(draft => {
      draft.enableMemory = enabled
    })
  }
