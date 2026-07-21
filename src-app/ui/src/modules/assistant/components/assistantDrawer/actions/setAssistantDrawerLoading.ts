import type { AssistantDrawerGet, AssistantDrawerSet } from '../state'

export default (set: AssistantDrawerSet, _get: AssistantDrawerGet) =>
  async (loading: boolean) => {
    set({ loading })
  }
