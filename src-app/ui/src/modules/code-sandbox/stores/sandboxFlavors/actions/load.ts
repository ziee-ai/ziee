import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import { toOptions, FALLBACK_OPTIONS, FALLBACK_HOST_COMMANDS } from '../state'
import type { SandboxFlavorsGet, SandboxFlavorsSet } from '../state'

export default (set: SandboxFlavorsSet, get: SandboxFlavorsGet) => async () => {
  if (get().loading) return
  // GET /api/code-sandbox/flavors is gated by code_sandbox::environments::read.
  // A user without it mounting a component reading this store would 403 — use
  // the fallback labels instead; the saved value is validated server-side.
  if (!hasPermissionNow(Permissions.CodeSandboxEnvironmentsRead)) {
    set(state => {
      state.selectOptions = FALLBACK_OPTIONS
      state.hostCommands = FALLBACK_HOST_COMMANDS
      state.isInitialized = true
    })
    return
  }
  set(state => {
    state.loading = true
    state.error = null
  })
  try {
    const res = await ApiClient.CodeSandbox.listFlavors()
    const flavors = res.available ?? []
    const hostCommands = res.host_allowed_commands ?? []
    set(state => {
      state.flavors = flavors
      state.selectOptions = toOptions(flavors)
      state.hostCommands = hostCommands
      state.loading = false
      state.isInitialized = true
    })
  } catch (err: any) {
    // Fall back to canonical labels so the form is usable.
    const errorMessage = err?.message ?? String(err)
    set(state => {
      state.flavors = []
      state.selectOptions = FALLBACK_OPTIONS
      state.hostCommands = FALLBACK_HOST_COMMANDS
      state.loading = false
      state.error = errorMessage
      state.isInitialized = true
    })
  }
}
