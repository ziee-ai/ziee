import { ApiClient } from '@/api-client'
import type { SandboxResourceLimitsGet, SandboxResourceLimitsSet } from '../state'

export default (set: SandboxResourceLimitsSet, _get: SandboxResourceLimitsGet) =>
  async () => {
    set(s => {
      s.loading = true
      s.error = null
    })
    try {
      const res = await ApiClient.CodeSandbox.getResourceLimits(undefined)
      set(s => {
        s.limits = res
        s.loading = false
      })
    } catch (e: any) {
      set(s => {
        s.error = e?.message ?? 'Failed to load resource limits'
        s.loading = false
      })
    }
  }
