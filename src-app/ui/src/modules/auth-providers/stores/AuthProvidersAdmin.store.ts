import { ApiClient } from '@/api-client'
import { type AuthProviderResponse, type CreateAuthProviderRequest, type TestProviderResponse, type UpdateAuthProviderRequest } from '@/api-client/types'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'
import {
  emitAuthProviderAutoDisabled,
  emitAuthProviderCreated,
  emitAuthProviderDeleted,
  emitAuthProviderUpdated,
} from '@/modules/auth-providers/events'

/**
 * Admin CRUD store for auth providers. Distinct from the public `AuthProviders`
 * store (login-page button row) — this drives the admin settings table. The
 * backend enforces "enabled requires a passing probe" and emits
 * `auth_provider.auto_disabled` so the Switch snaps back without a refresh.
 */
export const AuthProvidersAdmin = defineStore('AuthProvidersAdmin', {
  immer: true,
  state: {
    providers: [] as AuthProviderResponse[],
    // Start loading so the first paint shows a spinner, not a spurious empty
    // state, before init loads. loadProviders always resets on success/error.
    loading: true,
    saving: false,
    error: null as string | null,
    /// IDs currently mid-test (row Test button spinner). Cleared per-id.
    testingIds: new Set<string>(),
  },
  actions: (set, get) => {
    const loadProviders = async () => {
      // Self-gate: sync:reconnect fires for every store regardless of audience,
      // so a non-admin must not refetch this admin-only list (would 403).
      if (!hasPermissionNow(Permissions.AuthProvidersRead)) {
        // Clear the initial loading state so a non-admin mount doesn't hang.
        set(s => {
          s.loading = false
        })
        return
      }
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
    }
    return {
      loadProviders,
      createProvider: async (req: CreateAuthProviderRequest) => {
        set(s => {
          s.saving = true
          s.error = null
        })
        try {
          const created = await ApiClient.AuthProviders.create(req, undefined)
          set(s => {
            s.saving = false
          })
          try {
            await emitAuthProviderCreated(created.provider)
          } catch (eventError) {
            console.error('Failed to emit auth provider created event:', eventError)
          }
          if (created.connection_warning) {
            try {
              await emitAuthProviderAutoDisabled(created.provider.id, created.connection_warning)
            } catch (eventError) {
              console.error('Failed to emit auth provider auto_disabled event:', eventError)
            }
          }
          return created.provider
        } catch (e: any) {
          set(s => {
            s.error = e?.message ?? 'Failed to create provider'
            s.saving = false
          })
          throw e
        }
      },
      updateProvider: async (id: string, req: UpdateAuthProviderRequest) => {
        set(s => {
          s.saving = true
          s.error = null
        })
        try {
          const updated = await ApiClient.AuthProviders.update({ id, ...req }, undefined)
          set(s => {
            s.saving = false
          })
          try {
            await emitAuthProviderUpdated(updated)
          } catch (eventError) {
            console.error('Failed to emit auth provider updated event:', eventError)
          }
          return updated
        } catch (e: any) {
          set(s => {
            s.error = e?.message ?? 'Failed to update provider'
            s.saving = false
          })
          // Backend returns 400 AUTH_PROVIDER_ENABLE_FAILED_HEALTH_CHECK when an
          // enable-transition probe fails. Match the stable error_code (not a
          // message substring / `req.enabled`, which a dup-name 400 would
          // false-trip). The row is reverted server-side; the listener reloads.
          const code = (e as { error_code?: string })?.error_code
          if (code === 'AUTH_PROVIDER_ENABLE_FAILED_HEALTH_CHECK') {
            try {
              await emitAuthProviderAutoDisabled(
                id,
                typeof e?.message === 'string' ? e.message : 'Probe failed',
              )
            } catch (eventError) {
              console.error('Failed to emit auth provider auto_disabled event:', eventError)
            }
          }
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
            s.saving = false
          })
          try {
            await emitAuthProviderDeleted(id)
          } catch (eventError) {
            console.error('Failed to emit auth provider deleted event:', eventError)
          }
          return { affected_user_links: res.affected_user_links }
        } catch (e: any) {
          set(s => {
            s.error = e?.message ?? 'Failed to delete provider'
            s.saving = false
          })
          throw e
        }
      },
      testProvider: async (id: string): Promise<TestProviderResponse> => {
        set(s => {
          s.testingIds.add(id)
        })
        try {
          // Snapshot enabled BEFORE the test; the server may flip it.
          const wasEnabled = get().providers.find(p => p.id === id)?.enabled === true
          const res = await ApiClient.AuthProviders.test({ id }, undefined)
          // The originator's SSE self-echo guard suppresses the server's
          // sync_publish, so refresh inline to surface last_test_* + enabled.
          await loadProviders()
          set(s => {
            s.testingIds.delete(id)
          })
          if (wasEnabled && !res.ok) {
            try {
              await emitAuthProviderAutoDisabled(id, res.message)
            } catch (eventError) {
              console.error('Failed to emit auth provider auto_disabled event:', eventError)
            }
          }
          return res
        } catch (e: any) {
          set(s => {
            s.testingIds.delete(id)
          })
          return { ok: false, message: e?.message ?? 'Test failed' }
        }
      },
      testConfig: async (req: CreateAuthProviderRequest): Promise<TestProviderResponse> => {
        try {
          return await ApiClient.AuthProviders.testConfig(req, undefined)
        } catch (e: any) {
          return { ok: false, message: e?.message ?? 'Test failed' }
        }
      },
    }
  },
  init: ({ on, set, actions }) => {
    // In-process created/updated/deleted from local actions.
    on('auth_provider.created', event => {
      set(state => {
        state.providers.push(event.data.provider)
        state.providers.sort((a, b) => a.name.localeCompare(b.name))
      })
    })
    on('auth_provider.updated', event => {
      set(state => {
        const idx = state.providers.findIndex(p => p.id === event.data.provider.id)
        if (idx >= 0) state.providers[idx] = event.data.provider
      })
    })
    on('auth_provider.deleted', event => {
      set(state => {
        state.providers = state.providers.filter(p => p.id !== event.data.providerId)
        state.testingIds.delete(event.data.providerId)
      })
    })
    // Auto-disable: the backend (or another tab) flipped a row to enabled=false
    // because its probe failed. Reload so Switch + Alert reflect canonical state.
    on('auth_provider.auto_disabled', () => void actions.loadProviders())
    // Cross-device sync. loadProviders self-guards against in-flight loads.
    const reload = () => void actions.loadProviders()
    on('sync:auth_provider', reload)
    on('sync:reconnect', reload)
    void actions.loadProviders()
  },
})

export const useAuthProvidersAdminStore = AuthProvidersAdmin.store
