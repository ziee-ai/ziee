import type { BootstrapGet, BootstrapSet, BootstrapStatus } from '../state'

export default (set: BootstrapSet, _get: BootstrapGet) => {
  return async (status: BootstrapStatus, message: string | null = null) => {
    set({ status, message })
  }
}
