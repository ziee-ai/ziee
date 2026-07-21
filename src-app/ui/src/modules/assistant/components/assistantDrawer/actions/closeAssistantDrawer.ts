import type { AssistantDrawerGet, AssistantDrawerSet } from '../state'

export default (set: AssistantDrawerSet, _get: AssistantDrawerGet) =>
  async () => {
    set({
      open: false,
      loading: false,
      editingAssistant: null,
      isTemplate: false,
      isCloning: false,
    })
  }
