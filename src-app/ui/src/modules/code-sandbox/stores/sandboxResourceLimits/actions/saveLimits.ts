import { ApiClient } from '@/api-client'
import type { UpdateCodeSandboxResourceLimits } from '@/api-client/types'
import type { SandboxResourceLimitsGet, SandboxResourceLimitsSet } from '../state'

export default (set: SandboxResourceLimitsSet, _get: SandboxResourceLimitsGet) =>
  async (patch: UpdateCodeSandboxResourceLimits) => {
    set(s => {
      s.saving = true
      s.error = null
    })
    try {
      const res = await ApiClient.CodeSandbox.updateResourceLimits(patch)
      set(s => {
        s.limits = res
        s.saving = false
      })
    } catch (e: any) {
      set(s => {
        s.error = e?.message ?? 'Failed to save resource limits'
        s.saving = false
      })
      throw e
    }
  }
