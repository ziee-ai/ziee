import type { BootstrapGet, BootstrapSet, BootstrapStatus } from '../state'

export default (set: BootstrapSet, _get: BootstrapGet) => {
  return async () => {
    set({ status: 'idle' as BootstrapStatus, attempt: 0, message: null })
  }
}
