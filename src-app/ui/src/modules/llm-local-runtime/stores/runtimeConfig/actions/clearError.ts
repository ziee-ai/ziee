import type { RuntimeConfigGet, RuntimeConfigSet } from '../state'

export default (set: RuntimeConfigSet, _get: RuntimeConfigGet) =>
  () => {
    set(s => {
      s.error = null
    })
  }
