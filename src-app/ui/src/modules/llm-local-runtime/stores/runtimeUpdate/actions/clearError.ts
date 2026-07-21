import type { RuntimeUpdateGet, RuntimeUpdateSet } from '../state'

export default (set: RuntimeUpdateSet, _get: RuntimeUpdateGet) =>
  async () => {
    set({ error: null })
  }
