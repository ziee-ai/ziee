import type { BootstrapGet, BootstrapSet } from '../state'

export default (set: BootstrapSet, _get: BootstrapGet) => {
  return async (attempt: number) => {
    set({ attempt })
  }
}
