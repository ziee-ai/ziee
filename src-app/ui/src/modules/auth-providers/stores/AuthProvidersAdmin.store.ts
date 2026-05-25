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
 * Test results are persisted on the row (`last_test_at`,
 * `last_test_ok`, `last_test_message` — migration 48), so the table
 * reads them directly off the provider row. After a Test action we
 * just refresh the list to pick up the new values; no separate
 * in-memory cache.
 */
interface AuthProvidersAdminStore {
  providers: AuthProviderResponse[]
  loading: boolean
  saving: boolean
  error: string | null
  /// IDs currently mid-test, so the row's Test button can show a
  /// spinner. Cleared per-id when the call returns.
  testingIds: Set<string>

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
  /// Test a saved provider. Server persists the result on the row;
  /// we refresh the list afterwards so the UI shows the new values.
  testProvider: (id: string) => Promise<TestProviderResponse>
  /// Test a config payload WITHOUT saving it to the DB. Used by the
  /// EditDrawer's "Test config" button so admins can verify before
  /// committing.
  testConfig: (req: CreateAuthProviderRequest) => Promise<TestProviderResponse>
}

export const useAuthProvidersAdminStore = create<AuthProvidersAdminStore>()(
  subscribeWithSelector(
    immer((set, get) => ({
      providers: [],
      loading: false,
      saving: false,
      error: null,
      testingIds: new Set<string>(),

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
            s.testingIds.delete(id)
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
          s.testingIds.add(id)
        })
        try {
          const res = await ApiClient.AuthProviders.test({ id }, undefined)
          // Server persists the result on the row; reload so the
          // table renders the new last_test_at/ok/message immediately.
          await get().loadProviders()
          set(s => {
            s.testingIds.delete(id)
          })
          return res
        } catch (e: any) {
          set(s => {
            s.testingIds.delete(id)
          })
          return {
            ok: false,
            message: e?.message ?? 'Test failed',
          }
        }
      },

      testConfig: async (req: CreateAuthProviderRequest) => {
        try {
          return await ApiClient.AuthProviders.testConfig(req, undefined)
        } catch (e: any) {
          return {
            ok: false,
            message: e?.message ?? 'Test failed',
          }
        }
      },
    })),
  ),
)
