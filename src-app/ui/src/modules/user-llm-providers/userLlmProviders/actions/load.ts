import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { UserLlmProvidersGet, UserLlmProvidersSet } from '../state'
import doLoadFactory from './_doLoad'

export default (set: UserLlmProvidersSet, get: UserLlmProvidersGet) => {
  const doLoad = doLoadFactory(set, get)
  return async () => {
    // Permission-gate the shell-eager-load fetch: the chat model selector
    // accesses this store on every chat render. Without
    // user_llm_providers::read the parallel GETs 403 for restricted users.
    if (!hasPermissionNow(Permissions.UserLlmProvidersRead)) return
    set(state => {
      state.loading = true
      state.error = null
    })
    await doLoad()
  }
}
