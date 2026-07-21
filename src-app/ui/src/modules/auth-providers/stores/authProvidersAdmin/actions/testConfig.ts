import { ApiClient } from '@/api-client'
import { type CreateAuthProviderRequest, type TestProviderResponse } from '@/api-client/types'
import type { AuthProvidersAdminGet, AuthProvidersAdminSet } from '../state'

export default (_set: AuthProvidersAdminSet, _get: AuthProvidersAdminGet) =>
  async (req: CreateAuthProviderRequest): Promise<TestProviderResponse> => {
    try {
      return await ApiClient.AuthProviders.testConfig(req, undefined)
    } catch (e: any) {
      return { ok: false, message: e?.message ?? 'Test failed' }
    }
  }
