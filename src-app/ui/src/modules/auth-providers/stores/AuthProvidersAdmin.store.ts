import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import type {
  AuthProviderResponse,
  CreateAuthProviderRequest,
  TestProviderResponse,
  UpdateAuthProviderRequest,
} from '@/api-client/types'

/**
 * Admin CRUD store for auth providers. Distinct from the public
 * `AuthProviders` store in modules/auth — that one feeds the
 * login-page button row; this one drives the admin settings table.
 *
 * Per-row test results live in `testResults` keyed by provider id,
 * so the table can render an inline "✓ ok" / "✗ <reason>" badge
 * without scattering test state across components.
 */
interface AuthProvidersAdminStore {
  providers: AuthProviderResponse[]
  loading: boolean
  saving: boolean
  error: string | null
  testResults: Record<string, TestProviderResponse>
  testingId: string | null

  __init__: {
    providers?: () => Promise<void>
  }

  loadProviders: () => Promise<void>
  createProvider: (req: CreateAuthProviderRequest) => Promise<AuthProviderResponse>
  updateProvider: (
    id: string,
    req: UpdateAuthProviderRequest,
  ) => Promise<AuthProviderResponse>
  deleteProvider: (id: string) => Promise<{ affected_user_links: number }>
  testProvider: (id: string) => Promise<TestProviderResponse>
}

export const useAuthProvidersAdminStore = create<AuthProvidersAdminStore>()(
  subscribeWithSelector(
    immer((set, get) => ({
      providers: [],
      loading: false,
      saving: false,
      error: null,
      testResults: {},
      testingId: null,

      __init__: {
        providers: async () => {
          await get().loadProviders()
        },
      },

      loadProviders: async () => {
        set(s => {
          s.loading = true
          s.error = null
        })
        try {
          const res = await ApiClient.AuthProviders.list(undefined, undefined)
          set(s => {
            s.providers = res
            s.loading = false
          })
        } catch (e: any) {
          set(s => {
            s.error = e?.message ?? 'Failed to load providers'
            s.loading = false
          })
        }
      },

      createProvider: async (req: CreateAuthProviderRequest) => {
        set(s => {
          s.saving = true
          s.error = null
        })
        try {
          const created = await ApiClient.AuthProviders.create(req, undefined)
          set(s => {
            s.providers.push(created)
            s.providers.sort((a, b) => a.name.localeCompare(b.name))
            s.saving = false
          })
          return created
        } catch (e: any) {
          set(s => {
            s.error = e?.message ?? 'Failed to create provider'
            s.saving = false
          })
          throw e
        }
      },

      updateProvider: async (id, req) => {
        set(s => {
          s.saving = true
          s.error = null
        })
        try {
          const updated = await ApiClient.AuthProviders.update(
            { id, ...req },
            undefined,
          )
          set(s => {
            const idx = s.providers.findIndex(p => p.id === id)
            if (idx >= 0) s.providers[idx] = updated
            s.saving = false
          })
          return updated
        } catch (e: any) {
          set(s => {
            s.error = e?.message ?? 'Failed to update provider'
            s.saving = false
          })
          throw e
        }
      },

      deleteProvider: async (id: string) => {
        set(s => {
          s.saving = true
          s.error = null
        })
        try {
          const res = await ApiClient.AuthProviders.delete({ id }, undefined)
          set(s => {
            s.providers = s.providers.filter(p => p.id !== id)
            delete s.testResults[id]
            s.saving = false
          })
          return { affected_user_links: res.affected_user_links }
        } catch (e: any) {
          set(s => {
            s.error = e?.message ?? 'Failed to delete provider'
            s.saving = false
          })
          throw e
        }
      },

      testProvider: async (id: string) => {
        set(s => {
          s.testingId = id
        })
        try {
          const res = await ApiClient.AuthProviders.test({ id }, undefined)
          set(s => {
            s.testResults[id] = res
            s.testingId = null
          })
          return res
        } catch (e: any) {
          const res: TestProviderResponse = {
            ok: false,
            message: e?.message ?? 'Test failed',
          }
          set(s => {
            s.testResults[id] = res
            s.testingId = null
          })
          return res
        }
      },
    })),
  ),
)
